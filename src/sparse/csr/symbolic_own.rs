use super::*;
use crate::sparse::csc::*;

/// Symbolic structure of sparse matrix in row format, either compressed or uncompressed.
///
/// Requires:
/// * `nrows <= I::Signed::MAX` (always checked)
/// * `ncols <= I::Signed::MAX` (always checked)
/// * `row_ptrs` has length `nrows + 1` (always checked)
/// * `row_ptrs` is non-decreasing
/// * `row_ptrs[0]..row_ptrs[nrows]` is a valid range in row_indices (always checked, assuming
///   non-decreasing)
/// * if `nnz_per_row` is `None`, elements of `col_indices[row_ptrs[i]..row_ptrs[i + 1]]` are less
///   than `ncols`
///
/// * `nnz_per_row[i] <= row_ptrs[i+1] - row_ptrs[i]`
/// * if `nnz_per_row` is `Some(_)`, elements of `col_indices[row_ptrs[i]..][..nnz_per_row[i]]` are
///   less than `ncols`
///
/// * Within each row, column indices are sorted in non-decreasing order.
///
/// # Note
/// Some algorithms allow working with matrices containing unsorted row indices per column.
///
/// Passing such a matrix to an algorithm that does not explicitly permit this is unspecified
/// (though not undefined) behavior.

#[derive(Clone)]
pub struct SymbolicSparseRowMat<I: Index, R: Shape = usize, C: Shape = usize> {
    pub(crate) nrows: R,
    pub(crate) ncols: C,
    pub(crate) row_ptr: alloc::vec::Vec<I>,
    pub(crate) row_nnz: Option<alloc::vec::Vec<I>>,
    pub(crate) col_ind: alloc::vec::Vec<Idx<C, I>>,
}

impl<I: Index, R: Shape, C: Shape> SymbolicSparseRowMat<I, R, C> {
    /// Creates a new symbolic matrix view after asserting its invariants.
    ///
    /// # Panics
    ///
    /// See type level documentation.
    #[inline]
    #[track_caller]
    pub fn new_checked(
        nrows: R,
        ncols: C,
        row_ptrs: alloc::vec::Vec<I>,
        nnz_per_row: Option<alloc::vec::Vec<I>>,
        col_indices: alloc::vec::Vec<Idx<C, I>>,
    ) -> Self {
        SymbolicSparseRowMatRef::new_checked(
            nrows,
            ncols,
            &row_ptrs,
            nnz_per_row.as_deref(),
            &col_indices,
        );

        Self {
            nrows,
            ncols,
            row_ptr: row_ptrs,
            row_nnz: nnz_per_row,
            col_ind: col_indices,
        }
    }

    /// Creates a new symbolic matrix view from data containing duplicate and/or unsorted column
    /// indices per row, after asserting its other invariants.
    ///
    /// # Panics
    ///
    /// See type level documentation.
    #[inline]
    #[track_caller]
    pub fn new_unsorted_checked(
        nrows: R,
        ncols: C,
        row_ptrs: alloc::vec::Vec<I>,
        nnz_per_row: Option<alloc::vec::Vec<I>>,
        col_indices: alloc::vec::Vec<Idx<C, I>>,
    ) -> Self {
        SymbolicSparseRowMatRef::new_unsorted_checked(
            nrows,
            ncols,
            &row_ptrs,
            nnz_per_row.as_deref(),
            &col_indices,
        );

        Self {
            nrows,
            ncols,
            row_ptr: row_ptrs,
            row_nnz: nnz_per_row,
            col_ind: col_indices,
        }
    }

    /// Creates a new symbolic matrix view without asserting its invariants.
    ///
    /// # Safety
    ///
    /// See type level documentation.
    #[inline(always)]
    #[track_caller]
    pub unsafe fn new_unchecked(
        nrows: R,
        ncols: C,
        row_ptrs: alloc::vec::Vec<I>,
        nnz_per_row: Option<alloc::vec::Vec<I>>,
        col_indices: alloc::vec::Vec<Idx<C, I>>,
    ) -> Self {
        SymbolicSparseRowMatRef::new_unchecked(
            nrows,
            ncols,
            &row_ptrs,
            nnz_per_row.as_deref(),
            &col_indices,
        );

        Self {
            nrows,
            ncols,
            row_ptr: row_ptrs,
            row_nnz: nnz_per_row,
            col_ind: col_indices,
        }
    }

    /// Returns the components of the matrix in the order:
    /// - row count,
    /// - column count,
    /// - row pointers,
    /// - nonzeros per row,
    /// - column indices.
    #[inline]
    pub fn into_parts(
        self,
    ) -> (
        R,
        C,
        alloc::vec::Vec<I>,
        Option<alloc::vec::Vec<I>>,
        alloc::vec::Vec<Idx<C, I>>,
    ) {
        (
            self.nrows,
            self.ncols,
            self.row_ptr,
            self.row_nnz,
            self.col_ind,
        )
    }

    /// Returns a view over the symbolic structure of `self`.
    #[inline]
    pub fn as_ref(&self) -> SymbolicSparseRowMatRef<'_, I, R, C> {
        SymbolicSparseRowMatRef {
            nrows: self.nrows,
            ncols: self.ncols,
            row_ptr: &self.row_ptr,
            row_nnz: self.row_nnz.as_deref(),
            col_ind: &self.col_ind,
        }
    }

    /// Returns the number of rows of the matrix.
    #[inline]
    pub fn nrows(&self) -> R {
        self.nrows
    }
    /// Returns the number of columns of the matrix.
    #[inline]
    pub fn ncols(&self) -> C {
        self.ncols
    }

    /// Returns the number of rows and columns of the matrix.
    #[inline]
    pub fn shape(&self) -> (R, C) {
        (self.nrows(), self.ncols())
    }

    /// Consumes the matrix, and returns its transpose in column-major format without reallocating.
    ///
    /// # Note
    /// Allows unsorted matrices, producing an unsorted output.
    #[inline]
    pub fn into_transpose(self) -> SymbolicSparseColMat<I, C, R> {
        SymbolicSparseColMat {
            nrows: self.ncols,
            ncols: self.nrows,
            col_ptr: self.row_ptr,
            col_nnz: self.row_nnz,
            row_ind: self.col_ind,
        }
    }

    /// Copies the current matrix into a newly allocated matrix.
    ///
    /// # Note
    /// Allows unsorted matrices, producing an unsorted output.
    #[inline]
    pub fn to_owned(&self) -> Result<SymbolicSparseRowMat<I, R, C>, FaerError> {
        self.as_ref().to_owned()
    }

    /// Copies the current matrix into a newly allocated matrix, with column-major order.
    ///
    /// # Note
    /// Allows unsorted matrices, producing a sorted output. Duplicate entries are kept, however.
    #[inline]
    pub fn to_col_major(&self) -> Result<SymbolicSparseColMat<I, R, C>, FaerError> {
        self.as_ref().to_col_major()
    }

    /// Returns the number of symbolic non-zeros in the matrix.
    ///
    /// The value is guaranteed to be less than `I::Signed::MAX`.
    ///
    /// # Note
    /// Allows unsorted matrices, but the output is a count of all the entries, including the
    /// duplicate ones.
    #[inline]
    pub fn compute_nnz(&self) -> usize {
        self.as_ref().compute_nnz()
    }

    /// Returns the column pointers.
    #[inline]
    pub fn row_ptrs(&self) -> &[I] {
        &self.row_ptr
    }

    /// Returns the count of non-zeros per row of the matrix.
    #[inline]
    pub fn nnz_per_row(&self) -> Option<&[I]> {
        self.row_nnz.as_deref()
    }

    /// Returns the column indices.
    #[inline]
    pub fn col_indices(&self) -> &[Idx<C, I>] {
        &self.col_ind
    }

    /// Returns the column indices of row `i`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.nrows()`.
    #[inline]
    #[track_caller]
    pub fn col_indices_of_row_raw(&self, i: Idx<R>) -> &[Idx<C, I>] {
        self.as_ref().col_indices_of_row_raw(i)
    }

    /// Returns the column indices of row `i`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.ncols()`.
    #[inline]
    #[track_caller]
    pub fn col_indices_of_row(
        &self,
        i: Idx<R>,
    ) -> impl '_ + ExactSizeIterator + DoubleEndedIterator<Item = Idx<C>> {
        self.as_ref().col_indices_of_row(i)
    }

    /// Returns the range that the row `i` occupies in `self.col_indices()`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.nrows()`.
    #[inline]
    #[track_caller]
    pub fn row_range(&self, i: Idx<R>) -> Range<usize> {
        self.as_ref().row_range(i)
    }

    /// Returns the range that the row `i` occupies in `self.col_indices()`.
    ///
    /// # Safety
    ///
    /// The behavior is undefined if `i >= self.nrows()`.
    #[inline]
    #[track_caller]
    pub unsafe fn row_range_unchecked(&self, i: Idx<R>) -> Range<usize> {
        self.as_ref().row_range_unchecked(i)
    }

    /// Create a new symbolic structure, and the corresponding order for the numerical values
    /// from pairs of indices `(row, col)`.
    #[inline]
    pub fn try_new_from_indices(
        nrows: R,
        ncols: C,
        indices: &[(Idx<R, I>, Idx<C, I>)],
    ) -> Result<(Self, ValuesOrder<I>), CreationError> {
        SymbolicSparseColMat::try_new_from_indices_impl(
            ncols,
            nrows,
            |i| {
                let (row, col) = indices[i];
                (col, row)
            },
            indices.len(),
        )
        .map(|(m, o)| (m.into_transpose(), o))
    }
}

impl<I: Index, R: Shape, C: Shape> core::fmt::Debug for SymbolicSparseRowMat<I, R, C> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl<I: Index> SymbolicSparseRowMat<I> {
    /// Create a new symbolic structure, and the corresponding order for the numerical values
    /// from pairs of indices `(row, col)`.
    ///
    /// Negative indices are ignored.
    #[inline]
    pub fn try_new_from_nonnegative_indices(
        nrows: usize,
        ncols: usize,
        indices: &[(I::Signed, I::Signed)],
    ) -> Result<(Self, ValuesOrder<I>), CreationError> {
        SymbolicSparseColMat::try_new_from_nonnegative_indices_impl(
            ncols,
            nrows,
            |i| {
                let (row, col) = indices[i];
                (col, row)
            },
            indices.len(),
        )
        .map(|(m, o)| (m.into_transpose(), o))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbolic_sparse_row_mat_new_checked() {
        let nrows = 4;
        let ncols = 10;
        let row_ptrs = vec![0_u32, 4, 6, 6, 8];
        let col_indices = vec![0_u32, 1, 3, 5, 0, 3, 4, 5];
        let new_checked =
            SymbolicSparseRowMat::new_checked(nrows, ncols, row_ptrs, None, col_indices);
        assert_eq!(new_checked.nrows, 4);
    }

    #[test]
    fn test_symbolic_sparse_row_mat_new_unsorted_checked() {
        let nrows = 4;
        let ncols = 10;
        let row_ptrs = vec![0_u32, 4, 6, 6, 8];
        let col_indices = vec![0_u32, 1, 3, 5, 0, 3, 4, 5];
        let new_checked =
            SymbolicSparseRowMat::new_unsorted_checked(nrows, ncols, row_ptrs, None, col_indices);
        assert_eq!(new_checked.nrows, 4);
    }
}
