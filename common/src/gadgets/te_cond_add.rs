use ark_ec::{AffineRepr, CurveGroup};
use ark_ec::twisted_edwards::{Affine,TECurveConfig};
use ark_ff::{FftField, Field};
use ark_poly::{Evaluations, GeneralEvaluationDomain};
use ark_poly::univariate::DensePolynomial;
use ark_std::{vec, vec::Vec};

use crate::{Column, FieldColumn, const_evals};
use crate::domain::Domain;
use crate::gadgets::{ProverGadget, VerifierGadget};
use crate::gadgets::booleanity::BitColumn;
use crate::gadgets::cond_add::{AffineColumn, CondAdd, CondAddValues};

// Conditional affine addition:
// if the bit is set for a point, add the point to the acc and store,
// otherwise copy the acc value
pub struct TeCondAdd<F: FftField, P: AffineRepr<BaseField=F>> {
    pub(super)bitmask: BitColumn<F>,
    pub(super)points: AffineColumn<F, P>,
    // The polynomial `X - w^{n-1}` in the Lagrange basis
    pub(super)not_last: FieldColumn<F>,
    // Accumulates the (conditional) rolling sum of the points
    pub acc: AffineColumn<F, P>,
    pub result: P,
}

pub struct TeCondAddValues<F: Field> {
    pub bitmask: F,
    pub points: (F, F),
    pub not_last: F,
    pub acc: (F, F),
}

impl<F, Curve> CondAdd<F, Curve, Affine<Curve>> for TeCondAdd<F, Affine<Curve>> where
    F: FftField,
    Curve: TECurveConfig<BaseField=F>,
{
    type CondAddValT = TeCondAddValues<F>;
    // Populates the acc column starting from the supplied seed (as 0 doesn't work with the addition formula).
    // As the TE addition formula used is not complete, the seed must be selected in a way that would prevent
    // exceptional cases (doublings or adding the opposite point).
    // The last point of the input column is ignored, as adding it would made the acc column overflow due the initial point.
    fn init(bitmask: BitColumn<F>,
                points: AffineColumn<F, Affine<Curve>>,
                seed: Affine<Curve>,
                domain: &Domain<F>) -> Self {
        assert_eq!(bitmask.bits.len(), domain.capacity - 1);
        assert_eq!(points.points.len(), domain.capacity - 1);
        let not_last = domain.not_last_row.clone();
        let acc = bitmask.bits.iter()
            .zip(points.points.iter())
            .scan(seed, |acc, (&b, point)| {
                if b {
                    *acc = (*acc + point).into_affine();
                }
                Some(*acc)
            });
        let acc: Vec<_> = ark_std::iter::once(seed)
            .chain(acc)
            .collect();
        let init_plus_result = acc.last().unwrap();
        let result = init_plus_result.into_group() - seed.into_group();
        let result = result.into_affine();
        let acc = AffineColumn::private_column(acc, domain);

        Self { bitmask, points, acc, not_last, result }
    }

    fn evaluate_assignment(&self, z: &F) -> TeCondAddValues<F> {
        TeCondAddValues {
            bitmask: self.bitmask.evaluate(z),
            points: self.points.evaluate(z),
            not_last: self.not_last.evaluate(z),
            acc: self.acc.evaluate(z),
        }
    }
}

impl<F, Curve> ProverGadget<F> for TeCondAdd<F, Affine<Curve>>
    where
        F: FftField,
        Curve: TECurveConfig<BaseField=F>,
{
    fn witness_columns(&self) -> Vec<DensePolynomial<F>> {
        vec![self.acc.xs.poly.clone(), self.acc.ys.poly.clone()]
    }

    fn constraints(&self) -> Vec<Evaluations<F>> {
        let domain = self.bitmask.domain_4x();
        let b = &self.bitmask.col.evals_4x;
        let one = &const_evals(F::one(), domain);
        let te_a_coeff = &const_evals(Curve::COEFF_A, domain);
        let (x1, y1) = (&self.acc.xs.evals_4x, &self.acc.ys.evals_4x);
        let (x2, y2) = (&self.points.xs.evals_4x, &self.points.ys.evals_4x);
        let (x3, y3) = (&self.acc.xs.shifted_4x(), &self.acc.ys.shifted_4x());

        //b (x_3 (y_1 y_2 + ax_1 x_2) - x_1 y_1 - y_2 x_2) + (1 - b) (x_3 - x_1) = 0
        let mut c1 =
            &(
                b *
                    &(
                        &(
                            x3 *
                                &(
                                    &(y1 * y2) +
                                  
                                        &(te_a_coeff *

                                          &(x1 * x2)
                                        )
                                  
                                )
                        )
                            -
                             
                            &(
                                &(x1 * y1) + &(y2* x2)
                            )
                    )
            ) +
            &(
                &(one - b) * &(x3 - x1)
            );
        //b (y_3 (x_1 y_2 - x_2 y_1) - x_1 y_1 + x_2 y_2) + (1 - b) (y_3 - y_1) = 0
        let mut c2 =
            &(
                b *
                    &( &(y3 *
                         &(
                             &(x1 * y2) - &(x2 * y1)))  -                       
                         &(&(x1 * y1) - &(x2 * y2))
                    )
            )
                    +
                &(
                    &(one - b) * &(y3 - y1)
                );

        let not_last = &self.not_last.evals_4x;
        c1 *= not_last;
        c2 *= not_last;

        vec![c1, c2]
    }

    fn constraints_linearized(&self, z: &F) -> Vec<DensePolynomial<F>> {
        let vals = self.evaluate_assignment(z);
        let acc_x = self.acc.xs.as_poly();
        let acc_y = self.acc.ys.as_poly();

        let (c_acc_x, c_acc_y) = vals.acc_coeffs_1();
        let c1_lin = acc_x * c_acc_x + acc_y * c_acc_y;

        let (c_acc_x, c_acc_y) = vals.acc_coeffs_2();
        let c2_lin = acc_x * c_acc_x + acc_y * c_acc_y;

        vec![c1_lin, c2_lin]
    }

    fn domain(&self) -> GeneralEvaluationDomain<F> {
        self.bitmask.domain()
    }
}

impl<F: Field> VerifierGadget<F> for TeCondAddValues<F> {
    fn evaluate_constraints_main(&self) -> Vec<F> {
        let b = self.bitmask;
        let (x1, y1) = self.acc;
        let (x2, y2) = self.points;
        let (x3, y3) = (F::zero(), F::zero());

        let mut c1 =
            b * (
                (x1 - x2) * (x1 - x2) * (x1 + x2 + x3)
                    - (y2 - y1) * (y2 - y1)
            ) + (F::one() - b) * (y3 - y1);

        let mut c2 =
            b * (
                (x1 - x2) * (y3 + y1)
                    - (y2 - y1) * (x3 - x1)
            ) + (F::one() - b) * (x3 - x1);

        c1 *= self.not_last;
        c2 *= self.not_last;

        vec![c1, c2]
    }
}


impl<F: Field> CondAddValues<F> for TeCondAddValues<F> {
    fn acc_coeffs_1(&self) -> (F, F) {
        let b = self.bitmask;
        let (x1, _y1) = self.acc;
        let (x2, _y2) = self.points;

        let mut c_acc_x = b * (x1 - x2) * (x1 - x2);
        let mut c_acc_y = F::one() - b;

        c_acc_x *= self.not_last;
        c_acc_y *= self.not_last;

        (c_acc_x, c_acc_y)
    }

    fn acc_coeffs_2(&self) -> (F, F) {
        let b = self.bitmask;
        let (x1, y1) = self.acc;
        let (x2, y2) = self.points;

        let mut c_acc_x = b * (y1 - y2) + F::one() - b;
        let mut c_acc_y = b * (x1 - x2);

        c_acc_x *= self.not_last;
        c_acc_y *= self.not_last;

        (c_acc_x, c_acc_y)
    }
}


#[cfg(test)]
mod tests {
    use ark_ed_on_bls12_381_bandersnatch::EdwardsAffine;
    use ark_poly::Polynomial;
    use ark_std::test_rng;

    use crate::test_helpers::*;
    use crate::test_helpers::cond_sum;

    use super::*;

    fn _test_te_cond_add_gadget(hiding: bool) {
        let rng = &mut test_rng();

        let log_n = 10;
        let n = 2usize.pow(log_n);
        let domain = Domain::new(n, hiding);
        let seed = EdwardsAffine::generator();

        let bitmask = random_bitvec(domain.capacity - 1, 0.5, rng);
        let points = random_vec::<EdwardsAffine, _>(domain.capacity - 1, rng);
        let expected_res = seed + cond_sum(&bitmask, &points);

        let bitmask_col = BitColumn::init(bitmask, &domain);
        let points_col = AffineColumn::private_column(points, &domain);
        let gadget = TeCondAdd::init(bitmask_col, points_col, seed, &domain);
        let res = gadget.acc.points.last().unwrap();
        assert_eq!(res, &expected_res);

        let cs = gadget.constraints();
        let (c1, c2) = (&cs[0], &cs[1]);
        let c1 = c1.interpolate_by_ref();
        let c2 = c2.interpolate_by_ref();
        assert_eq!(c1.degree(), 4 * n - 3);
        assert_eq!(c2.degree(), 4 * n - 3);

        domain.divide_by_vanishing_poly(&c1);
        domain.divide_by_vanishing_poly(&c2);
        // test_gadget(gadget);
    }

    #[test]
    fn test_te_cond_add_gadget() {
        _test_te_cond_add_gadget(false);
        _test_te_cond_add_gadget(true);
    }
}
