use ark_ec::AffineRepr;
use ark_ff::PrimeField;
use fflonk::pcs::PCS;

use common::gadgets::cond_add::CondAdd;
use common::gadgets::ProverGadget;
use common::prover::PlonkProver;

use crate::piop::params::PiopParams;
use crate::piop::{FixedColumns, PiopProver, ProverKey};
use crate::RingProof;

pub struct RingProver<F: PrimeField, CS: PCS<F>, P: AffineRepr<BaseField = F>> {
    piop_params: PiopParams<F, P>,
    fixed_columns: FixedColumns<F, P>,
    k: usize,
    plonk_prover: PlonkProver<F, CS, merlin::Transcript>,
}

impl<F: PrimeField, CS: PCS<F>, P: AffineRepr<BaseField = F>> RingProver<F, CS, P> {
    pub fn init(
        prover_key: ProverKey<F, CS, P>,
        piop_params: PiopParams<F, P>,
        k: usize,
        empty_transcript: merlin::Transcript,
    ) -> Self {
        let ProverKey {
            pcs_ck,
            fixed_columns,
            verifier_key,
        } = prover_key;

        let plonk_prover = PlonkProver::init(pcs_ck, verifier_key, empty_transcript);

        Self {
            piop_params,
            fixed_columns,
            k,
            plonk_prover,
        }
    }

    pub fn prove<CondAddT: CondAdd<F, P> + ProverGadget<F>>(
        &self,
        t: P::ScalarField,
    ) -> RingProof<F, CS> {
        let piop: PiopProver<F, P, CondAddT> =
            PiopProver::build(&self.piop_params, self.fixed_columns.clone(), self.k, t);
        self.plonk_prover.prove(piop)
    }

    pub fn piop_params(&self) -> &PiopParams<F, P> {
        &self.piop_params
    }
}
