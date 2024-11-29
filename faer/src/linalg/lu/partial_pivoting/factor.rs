use crate::{assert, internal_prelude::*, perm::swap_rows_idx};
use core::num::NonZero;

#[math]
fn swap_elems<T: ComplexField>(mut col: ColMut<'_, T>, i: usize, j: usize) {
    let a = copy(col[i]);
    let b = copy(col[j]);

    col[i] = b;
    col[j] = a;
}

#[math]
fn lu_in_place_unblocked<I: Index, T: ComplexField>(
    matrix: MatMut<'_, T>,
    start: usize,
    end: usize,
    trans: &mut [I],
) -> usize {
    let mut matrix = matrix;
    let m = matrix.nrows();

    if start == end {
        return 0;
    }

    let mut n_trans = 0;

    for j in start..end {
        let j = j - start;

        let t = &mut trans[j];
        let mut imax = j;
        let mut max = zero();

        for i in imax..m {
            let abs = abs1(matrix[(i, j)]);
            if abs > max {
                max = abs;
                imax = i;
            }
        }

        *t = I::truncate(imax - j);

        if imax != j {
            swap_rows_idx(matrix.rb_mut(), j, imax);
            n_trans += 1;
        }

        let mut matrix = matrix.rb_mut().get_mut(.., start..end);

        let inv = recip(matrix[(j, j)]);
        for i in j + 1..m {
            matrix[(i, j)] = matrix[(i, j)] * inv;
        }

        let (_, A01, A10, A11) = matrix.rb_mut().split_at_mut(j + 1, j + 1);
        let A01 = A01.row(j);
        let A10 = A10.col(j);
        linalg::matmul::matmul(
            A11,
            Accum::Add,
            A10.as_mat(),
            A01.as_mat(),
            -one::<T>(),
            Par::Seq,
        );
    }

    n_trans
}

#[math]
fn lu_in_place_recursion<I: Index, T: ComplexField>(
    A: MatMut<'_, T>,
    start: usize,
    end: usize,
    trans: &mut [I],
    par: Par,
    params: PartialPivLuParams,
) -> usize {
    let mut A = A;
    let m = A.nrows();
    let ncols = A.ncols();
    let n = end - start;

    if n <= params.recursion_threshold.get() {
        return lu_in_place_unblocked(A, start, end, trans);
    }

    let blocksize = Ord::min(
        params.recursion_threshold.get(),
        Ord::max(params.blocksize.get(), n.next_power_of_two() / 2),
    );
    let blocksize = Ord::min(blocksize, n);

    let mut n_trans = 0;

    assert!(n <= m);

    n_trans += lu_in_place_recursion(
        A.rb_mut().get_mut(.., start..end),
        0,
        blocksize,
        &mut trans[..blocksize],
        par,
        params,
    );

    {
        let mut A = A.rb_mut().get_mut(.., start..end);
        let (A00, mut A01, A10, mut A11) = A.rb_mut().split_at_mut(blocksize, blocksize);

        let A00 = A00.rb();
        let A10 = A10.rb();
        {
            linalg::triangular_solve::solve_unit_lower_triangular_in_place(
                A00.rb(),
                A01.rb_mut(),
                par,
            );
        }

        linalg::matmul::matmul(
            A11.rb_mut(),
            Accum::Add,
            A10.rb(),
            A01.rb(),
            -one::<T>(),
            par,
        );

        n_trans += lu_in_place_recursion(
            A.rb_mut().get_mut(blocksize..m, ..),
            blocksize,
            n,
            &mut trans[blocksize..n],
            par,
            params,
        );
    }

    let swap = |mat: MatMut<'_, T>| {
        let mut mat = mat;
        for j in 0..mat.ncols() {
            let mut col = mat.rb_mut().col_mut(j);

            for j in 0..blocksize {
                let t = trans[j];
                swap_elems(col.rb_mut(), j, t.zx() + j);
            }

            for j in blocksize..n {
                let t = trans[j];
                swap_elems(col.rb_mut(), j, t.zx() + j);
            }
        }
    };

    let (A_left, A_right) = A.rb_mut().split_at_col_mut(start);
    let A_right = A_right.get_mut(.., end - start..ncols - start);

    match par {
        Par::Seq => {
            swap(A_left);
            swap(A_right);
        }
        #[cfg(feature = "rayon")]
        Par::Rayon(nthreads) => {
            let nthreads = nthreads.get();
            let len = (A_left.ncols() + A_right.ncols()) as f64;
            let left_threads = Ord::min(
                (nthreads as f64 * (A_left.ncols() as f64 / len)) as usize,
                nthreads,
            );
            let right_threads = nthreads - left_threads;

            use rayon::prelude::*;
            rayon::join(
                || {
                    A_left
                        .par_col_partition_mut(left_threads)
                        .for_each(|A| swap(A))
                },
                || {
                    A_right
                        .par_col_partition_mut(right_threads)
                        .for_each(|A| swap(A))
                },
            );
        }
    }

    n_trans
}

/// LUfactorization tuning parameters.
#[derive(Copy, Clone, Debug)]
pub struct PartialPivLuParams {
    pub recursion_threshold: NonZero<usize>,
    pub blocksize: NonZero<usize>,

    pub non_exhaustive: NonExhaustive,
}

/// Information about the resulting LU factorization.
#[derive(Copy, Clone, Debug)]
pub struct PartialPivLuInfo {
    /// Number of transpositions that were performed, can be used to compute the determinant of
    /// $P$.
    pub transposition_count: usize,
}

/// Error in the LDLT factorization.
#[derive(Copy, Clone, Debug)]
pub enum LdltError {
    ZeroPivot { index: usize },
}

impl<T: ComplexField> Auto<T> for PartialPivLuParams {
    #[inline]
    fn auto() -> Self {
        Self {
            recursion_threshold: NonZero::new(16).unwrap(),
            blocksize: NonZero::new(64).unwrap(),
            non_exhaustive: NonExhaustive(()),
        }
    }
}

#[inline]
pub fn lu_in_place_scratch<I: Index, T: ComplexField>(
    nrows: usize,
    ncols: usize,
    par: Par,
    params: PartialPivLuParams,
) -> Result<StackReq, SizeOverflow> {
    _ = par;
    _ = params;
    StackReq::try_new::<I>(Ord::min(nrows, ncols))
}

pub fn lu_in_place<'out, I: Index, T: ComplexField>(
    A: MatMut<'_, T>,
    perm: &'out mut [I],
    perm_inv: &'out mut [I],
    par: Par,
    stack: &mut DynStack,
    params: PartialPivLuParams,
) -> (PartialPivLuInfo, PermRef<'out, I>) {
    let _ = &params;
    let truncate = I::truncate;

    #[cfg(feature = "perf-warn")]
    if (A.col_stride().unsigned_abs() == 1 || A.row_stride().unsigned_abs() != 1)
        && crate::__perf_warn!(LU_WARN)
    {
        log::warn!(target: "faer_perf", "LU with partial pivoting prefers column-major or row-major matrix. Found matrix with generic strides.");
    }

    let mut matrix = A;
    let mut stack = stack;
    let m = matrix.nrows();
    let n = matrix.ncols();

    let size = Ord::min(n, m);

    for i in 0..m {
        let p = &mut perm[i];
        *p = truncate(i);
    }

    let (mut transpositions, _) = stack.rb_mut().make_with(size, |_| truncate(0));
    let transpositions = transpositions.as_mut();

    let n_transpositions = lu_in_place_recursion(
        matrix.rb_mut(),
        0,
        size,
        transpositions.as_mut(),
        par,
        params,
    );

    for idx in 0..size {
        let t = transpositions[idx];
        perm.as_mut().swap(idx, idx + t.zx());
    }

    if m < n {
        let (left, right) = matrix.split_at_col_mut(size);
        linalg::triangular_solve::solve_unit_lower_triangular_in_place(left.rb(), right, par);
    }

    for i in 0..m {
        perm_inv[perm[i].zx()] = truncate(i);
    }

    (
        PartialPivLuInfo {
            transposition_count: n_transpositions,
        },
        unsafe { PermRef::new_unchecked(perm, perm_inv, m) },
    )
}

#[cfg(test)]
mod tests {
    use dyn_stack::GlobalMemBuffer;

    use super::*;
    use crate::{assert, stats::prelude::*, utils::approx::*, Mat};

    #[test]
    fn test_plu() {
        let rng = &mut StdRng::seed_from_u64(0);

        let approx_eq = CwiseMat(ApproxEq {
            abs_tol: 1e-13,
            rel_tol: 1e-13,
        });

        for n in [1, 2, 3, 128, 255, 256, 257] {
            let A = CwiseMatDistribution {
                nrows: n,
                ncols: n,
                dist: StandardNormal,
            }
            .rand::<Mat<f64>>(rng);
            let A = A.as_ref();

            let mut LU = A.cloned();
            let perm = &mut *vec![0usize; n];
            let perm_inv = &mut *vec![0usize; n];

            let params = PartialPivLuParams {
                recursion_threshold: NonZero::new(2).unwrap(),
                blocksize: NonZero::new(2).unwrap(),
                ..auto!(f64)
            };
            let p = lu_in_place(
                LU.as_mut(),
                perm,
                perm_inv,
                Par::Seq,
                DynStack::new(&mut GlobalMemBuffer::new(
                    lu_in_place_scratch::<usize, f64>(n, n, Par::Seq, params).unwrap(),
                )),
                params,
            )
            .1;

            let mut L = LU.as_ref().cloned();
            let mut U = LU.as_ref().cloned();

            for j in 0..n {
                for i in 0..j {
                    L[(i, j)] = 0.0;
                }
                L[(j, j)] = 1.0;
            }
            for j in 0..n {
                for i in j + 1..n {
                    U[(i, j)] = 0.0;
                }
            }
            let L = L.as_ref();
            let U = U.as_ref();

            assert!(p.inverse() * L * U ~ A);
        }

        for m in [8, 128, 255, 256, 257] {
            let n = 8;

            let A = CwiseMatDistribution {
                nrows: m,
                ncols: n,
                dist: StandardNormal,
            }
            .rand::<Mat<f64>>(rng);
            let A = A.as_ref();

            let mut LU = A.cloned();
            let perm = &mut *vec![0usize; m];
            let perm_inv = &mut *vec![0usize; m];

            let p = lu_in_place(
                LU.as_mut(),
                perm,
                perm_inv,
                Par::Seq,
                DynStack::new(&mut GlobalMemBuffer::new(
                    lu_in_place_scratch::<usize, f64>(n, n, Par::Seq, auto!(f64)).unwrap(),
                )),
                auto!(f64),
            )
            .1;

            let mut L = LU.as_ref().cloned();
            let mut U = LU.as_ref().cloned();

            for j in 0..n {
                for i in 0..j {
                    L[(i, j)] = 0.0;
                }
                L[(j, j)] = 1.0;
            }
            for j in 0..n {
                for i in j + 1..m {
                    U[(i, j)] = 0.0;
                }
            }
            let L = L.as_ref();
            let U = U.as_ref();

            let U = U.subrows(0, n);

            assert!(p.inverse() * L * U ~ A);
        }
    }
}
