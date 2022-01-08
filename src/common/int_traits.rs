/// Returns `self + add_by - sub_by`.
pub trait AddAndSub<Rhs = Self> {
    type Output;
    fn add_and_sub(self, add_by: Rhs, sub_by: Rhs) -> Self::Output;
}

impl AddAndSub for usize {
    type Output = usize;

    fn add_and_sub(self, add_by: usize, sub_by: usize) -> usize {
        if add_by > sub_by {
            self + (add_by - sub_by)
        } else {
            self - (sub_by - add_by)
        }
    }
}
