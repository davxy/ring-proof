mod prover;
mod verifier;
pub mod params;

pub(crate) use prover::PiopProver;
pub(crate) use verifier::PiopVerifier;

use std::marker::PhantomData;
use ark_ff::{FftField, Field, PrimeField};
use ark_poly::{EvaluationDomain, GeneralEvaluationDomain};
use fflonk::pcs::Commitment;
use common::{Column, ColumnsCommited, ColumnsEvaluated, FieldColumn, l_first, l_last, not_last};
use ark_serialize::{CanonicalSerialize, CanonicalDeserialize};
use ark_serialize::{Read, Write, SerializationError};

#[derive(Clone)]
pub struct SelectorColumns<F: FftField> {
    ring_selector: FieldColumn<F>,
    l_first: FieldColumn<F>,
    l_last: FieldColumn<F>,
    not_last: FieldColumn<F>,
}

impl<F: FftField> SelectorColumns<F> {
    pub fn init(domain_size: usize, keyset_size: usize) -> Self {
        let p = domain_size - keyset_size;
        assert!(p > 0); //TODO
        let domain = GeneralEvaluationDomain::new(domain_size).unwrap();
        let ring_selector = [vec![F::one(); keyset_size], vec![F::zero(); p]].concat();
        let ring_selector = FieldColumn::init(ring_selector);
        let l_first = FieldColumn::init(l_first(domain_size));
        let l_last = FieldColumn::init(l_last(domain_size));
        // doesn't really require FFTs to compute, but who cares
        let not_last = FieldColumn::from_poly(not_last(domain), domain_size);
        SelectorColumns {
            ring_selector,
            l_first,
            l_last,
            not_last,
        }
    }

    pub fn evaluate(&self, zeta: &F) -> SelectorsValues<F> {
        SelectorsValues {
            ring_selector: self.ring_selector.evaluate(zeta),
            l_first: self.l_first.evaluate(zeta),
            l_last: self.l_last.evaluate(zeta),
            not_last: self.not_last.evaluate(zeta),
            n: self.l_last.domain().size(),
            omega: self.l_last.domain().group_gen(),
        }
    }
}


pub struct SelectorsValues<F: Field> {
    ring_selector: F,
    l_first: F,
    l_last: F,
    not_last: F,
    n: usize,
    omega: F,
}

#[derive(Clone, CanonicalSerialize, CanonicalDeserialize)]
pub struct RingCommitments<F: PrimeField, C: Commitment<F>> {
    pub(crate) bits: C,
    pub(crate) cond_add_acc: [C; 2],
    pub(crate) inn_prod_acc: C,
    pub(crate) phantom: PhantomData<F>,
}

impl<F: PrimeField, C: Commitment<F>> ColumnsCommited<F, C> for RingCommitments<F, C> {
    fn to_vec(self) -> Vec<C> {
        vec![
            self.bits,
            self.cond_add_acc[0].clone(),
            self.cond_add_acc[1].clone(),
            self.inn_prod_acc,
        ]
    }
}

#[derive(Clone, CanonicalSerialize, CanonicalDeserialize)]
pub struct RingEvaluations<F: PrimeField> {
    pub(crate) points: [F; 2],
    pub(crate) bits: F,
    pub(crate) cond_add_acc: [F; 2],
    pub(crate) inn_prod_acc: F,
}

impl<F: PrimeField> ColumnsEvaluated<F> for RingEvaluations<F> {
    fn to_vec(self) -> Vec<F> {
        vec![
            self.points[0],
            self.points[1],
            self.bits,
            self.cond_add_acc[0],
            self.cond_add_acc[1],
            self.inn_prod_acc,
        ]
    }
}