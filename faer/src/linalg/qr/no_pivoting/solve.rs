use crate::{assert, internal_prelude::*};

pub fn solve_lstsq_in_place_scratch<T: ComplexField>(
    qr_nrows: usize,
    qr_ncols: usize,
    qr_blocksize: usize,
    rhs_ncols: usize,
    par: Par,
) -> Result<StackReq, SizeOverflow> {
    _ = qr_ncols;
    _ = par;
    linalg::householder::apply_block_householder_sequence_transpose_on_the_left_in_place_scratch::<T>(
        qr_nrows,
        qr_blocksize,
        rhs_ncols,
    )
}

pub fn solve_in_place_scratch<T: ComplexField>(
    qr_dim: usize,
    qr_blocksize: usize,
    rhs_ncols: usize,
    par: Par,
) -> Result<StackReq, SizeOverflow> {
    solve_lstsq_in_place_scratch::<T>(qr_dim, qr_dim, qr_blocksize, rhs_ncols, par)
}

pub fn solve_transpose_in_place_scratch<T: ComplexField>(
    qr_dim: usize,
    qr_blocksize: usize,
    rhs_ncols: usize,
    par: Par,
) -> Result<StackReq, SizeOverflow> {
    _ = par;
    linalg::householder::apply_block_householder_sequence_on_the_left_in_place_scratch::<T>(
        qr_dim,
        qr_blocksize,
        rhs_ncols,
    )
}

#[track_caller]
pub fn solve_lstsq_in_place_with_conj<T: ComplexField>(
    QR: MatRef<'_, T>,
    H: MatRef<'_, T>,
    conj_QR: Conj,
    rhs: MatMut<'_, T>,
    par: Par,
    stack: &mut DynStack,
) {
    let m = QR.nrows();
    let n = QR.ncols();
    let size = Ord::min(m, n);
    let blocksize = H.nrows();
    assert!(all(
        QR.nrows() >= QR.ncols(),
        rhs.nrows() == m,
        H.ncols() == size,
        H.nrows() == blocksize,
    ));

    let mut rhs = rhs;
    let mut stack = stack;
    linalg::householder::apply_block_householder_sequence_transpose_on_the_left_in_place_with_conj(
        QR,
        H,
        conj_QR.compose(Conj::Yes),
        rhs.rb_mut(),
        par,
        stack.rb_mut(),
    );

    linalg::triangular_solve::solve_upper_triangular_in_place_with_conj(
        QR.submatrix(0, 0, size, size),
        conj_QR,
        rhs.subrows_mut(0, size),
        par,
    );
}

#[track_caller]
pub fn solve_lstsq_in_place<T: ComplexField, C: Conjugate<Canonical = T>>(
    QR: MatRef<'_, C>,
    H: MatRef<'_, C>,
    rhs: MatMut<'_, T>,
    par: Par,
    stack: &mut DynStack,
) {
    solve_lstsq_in_place_with_conj(
        QR.canonical(),
        H.canonical(),
        Conj::get::<C>(),
        rhs,
        par,
        stack,
    );
}

#[track_caller]
pub fn solve_in_place_with_conj<T: ComplexField>(
    QR: MatRef<'_, T>,
    H: MatRef<'_, T>,
    conj_QR: Conj,
    rhs: MatMut<'_, T>,
    par: Par,
    stack: &mut DynStack,
) {
    let n = QR.nrows();
    let blocksize = H.nrows();
    assert!(all(
        QR.ncols() == n,
        QR.nrows() == n,
        rhs.nrows() == n,
        H.ncols() == n,
        H.nrows() == blocksize,
    ));

    solve_lstsq_in_place_with_conj(QR, H, conj_QR, rhs, par, stack);
}

#[track_caller]
pub fn solve_in_place<T: ComplexField, C: Conjugate<Canonical = T>>(
    QR: MatRef<'_, C>,
    H: MatRef<'_, C>,
    rhs: MatMut<'_, T>,
    par: Par,
    stack: &mut DynStack,
) {
    solve_in_place_with_conj(
        QR.canonical(),
        H.canonical(),
        Conj::get::<C>(),
        rhs,
        par,
        stack,
    );
}

#[track_caller]
pub fn solve_transpose_in_place_with_conj<T: ComplexField>(
    QR: MatRef<'_, T>,
    H: MatRef<'_, T>,
    conj_QR: Conj,
    rhs: MatMut<'_, T>,
    par: Par,
    stack: &mut DynStack,
) {
    let n = QR.nrows();
    let blocksize = H.nrows();

    assert!(all(
        QR.ncols() == n,
        QR.nrows() == n,
        rhs.nrows() == n,
        H.ncols() == n,
        H.nrows() == blocksize,
    ));

    let mut rhs = rhs;
    let mut stack = stack;

    linalg::triangular_solve::solve_lower_triangular_in_place_with_conj(
        QR.transpose(),
        conj_QR,
        rhs.rb_mut(),
        par,
    );
    linalg::householder::apply_block_householder_sequence_on_the_left_in_place_with_conj(
        QR,
        H,
        conj_QR.compose(Conj::Yes),
        rhs.rb_mut(),
        par,
        stack.rb_mut(),
    );
}

#[track_caller]
pub fn solve_transpose_in_place<T: ComplexField, C: Conjugate<Canonical = T>>(
    QR: MatRef<'_, C>,
    H: MatRef<'_, C>,
    rhs: MatMut<'_, T>,
    par: Par,
    stack: &mut DynStack,
) {
    solve_transpose_in_place_with_conj(
        QR.canonical(),
        H.canonical(),
        Conj::get::<C>(),
        rhs,
        par,
        stack,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert, stats::prelude::*, utils::approx::*};
    use dyn_stack::GlobalMemBuffer;
    use linalg::qr::no_pivoting::*;

    #[test]
    fn test_lstsq() {
        let rng = &mut StdRng::seed_from_u64(0);
        let m = 100;
        let n = 50;
        let k = 3;

        let A = CwiseMatDistribution {
            nrows: m,
            ncols: n,
            dist: ComplexDistribution::new(StandardNormal, StandardNormal),
        }
        .rand::<Mat<c64>>(rng);

        let B = CwiseMatDistribution {
            nrows: m,
            ncols: k,
            dist: ComplexDistribution::new(StandardNormal, StandardNormal),
        }
        .rand::<Mat<c64>>(rng);

        let mut QR = A.to_owned();
        let mut H = Mat::zeros(4, n);

        factor::qr_in_place(
            QR.as_mut(),
            H.as_mut(),
            Par::Seq,
            DynStack::new(&mut GlobalMemBuffer::new(
                factor::qr_in_place_scratch::<c64>(m, n, 4, Par::Seq, auto!(c64)).unwrap(),
            )),
            auto!(c64),
        );

        let approx_eq = CwiseMat(ApproxEq::<c64>::eps() * (n as f64));

        {
            let mut X = B.to_owned();
            solve::solve_lstsq_in_place(
                QR.as_ref(),
                H.as_ref(),
                X.as_mut(),
                Par::Seq,
                DynStack::new(&mut GlobalMemBuffer::new(
                    solve::solve_lstsq_in_place_scratch::<c64>(m, n, 4, k, Par::Seq).unwrap(),
                )),
            );

            let X = X.get(..n, ..);

            assert!(A.adjoint() * &A * &X ~ A.adjoint() * &B);
        }

        {
            let mut X = B.to_owned();
            solve::solve_lstsq_in_place(
                QR.conjugate(),
                H.conjugate(),
                X.as_mut(),
                Par::Seq,
                DynStack::new(&mut GlobalMemBuffer::new(
                    solve::solve_lstsq_in_place_scratch::<c64>(m, n, 4, k, Par::Seq).unwrap(),
                )),
            );

            let X = X.get(..n, ..);
            assert!(A.transpose() * A.conjugate() * &X ~ A.transpose() * &B);
        }
    }

    #[test]
    fn test_solve() {
        let rng = &mut StdRng::seed_from_u64(0);
        let n = 50;
        let k = 3;

        let A = CwiseMatDistribution {
            nrows: n,
            ncols: n,
            dist: ComplexDistribution::new(StandardNormal, StandardNormal),
        }
        .rand::<Mat<c64>>(rng);

        let B = CwiseMatDistribution {
            nrows: n,
            ncols: k,
            dist: ComplexDistribution::new(StandardNormal, StandardNormal),
        }
        .rand::<Mat<c64>>(rng);

        let mut QR = A.to_owned();
        let mut H = Mat::zeros(4, n);

        factor::qr_in_place(
            QR.as_mut(),
            H.as_mut(),
            Par::Seq,
            DynStack::new(&mut GlobalMemBuffer::new(
                factor::qr_in_place_scratch::<c64>(n, n, 4, Par::Seq, auto!(c64)).unwrap(),
            )),
            auto!(c64),
        );

        let approx_eq = CwiseMat(ApproxEq::<c64>::eps() * (n as f64));

        {
            let mut X = B.to_owned();
            solve::solve_in_place(
                QR.as_ref(),
                H.as_ref(),
                X.as_mut(),
                Par::Seq,
                DynStack::new(&mut GlobalMemBuffer::new(
                    solve::solve_in_place_scratch::<c64>(n, 4, k, Par::Seq).unwrap(),
                )),
            );

            assert!(&A * &X ~ B);
        }

        {
            let mut X = B.to_owned();
            solve::solve_in_place(
                QR.conjugate(),
                H.conjugate(),
                X.as_mut(),
                Par::Seq,
                DynStack::new(&mut GlobalMemBuffer::new(
                    solve::solve_in_place_scratch::<c64>(n, 4, k, Par::Seq).unwrap(),
                )),
            );

            assert!(A.conjugate() * &X ~ B);
        }

        {
            let mut X = B.to_owned();
            solve::solve_transpose_in_place(
                QR.as_ref(),
                H.as_ref(),
                X.as_mut(),
                Par::Seq,
                DynStack::new(&mut GlobalMemBuffer::new(
                    solve::solve_transpose_in_place_scratch::<c64>(n, 4, k, Par::Seq).unwrap(),
                )),
            );

            assert!(A.transpose() * &X ~ B);
        }

        {
            let mut X = B.to_owned();
            solve::solve_transpose_in_place(
                QR.conjugate(),
                H.conjugate(),
                X.as_mut(),
                Par::Seq,
                DynStack::new(&mut GlobalMemBuffer::new(
                    solve::solve_transpose_in_place_scratch::<c64>(n, 4, k, Par::Seq).unwrap(),
                )),
            );

            assert!(A.adjoint() * &X ~ B);
        }
    }
}
