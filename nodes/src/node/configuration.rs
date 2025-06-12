use crate::crypto::{
    crypto_lib::evaluation_domain::{BatchEvaluationDomain, EvaluationDomain},
    data_structures::Base,
};
use global_lib::{config_treatment::fields::Fields, messages::Algo, OpId, Step};

#[derive(Debug, Clone)]
pub struct Configuration {
    algo: Algo,
    t: u16,
    l: u16,
    n: u16,
    r: u16,
    dealer_corruption: bool,
    id: OpId,
    step: Step,
    index: u16,
    dealer: u16,
    is_byz: bool,
    batch_size: usize,
    dom: EvaluationDomain,
    batch_dom: BatchEvaluationDomain,
    base: Base,
}

impl Configuration {
    pub fn from_fields(fields: &mut Fields, base: Base, index: u16, id: OpId, dealer: u16) -> Self {
        if matches!(fields.algo(), Algo::DualAvssSimpl) {
            fields.set_l(100);
        }

        assert!(fields.dealer_corruption() <= 1);
        let domain_size = fields.algo().domain_size(fields.n(), fields.batch_size()) as usize;
        let batch_dom = BatchEvaluationDomain::new(domain_size);
        let dom = batch_dom.get_subdomain(domain_size);
        Configuration {
            base,
            algo: fields.algo(),
            t: fields.t(),
            l: fields.l(),
            n: fields.n(),
            step: fields.step(),
            dealer_corruption: fields.dealer_corruption() == 1,
            r: (2 * fields.t() - fields.l()),
            is_byz: index < fields.nb_byz(),
            batch_size: fields.batch_size() as usize,
            id,
            index,
            dealer,
            batch_dom,
            dom,
        }
    }

    pub fn base(&self) -> &Base {
        &self.base
    }

    pub fn algo(&self) -> Algo {
        self.algo
    }

    pub fn step(&self) -> Step {
        self.step
    }

    pub fn id(&self) -> OpId {
        self.id
    }

    pub fn r(&self) -> u16 {
        self.r
    }

    pub fn get_threshold(&self) -> usize {
        self.degree() as usize + 1
    }

    pub fn change_byz_comp(&mut self, is_byz: bool) {
        self.is_byz = is_byz;
    }

    pub fn t(&self) -> u16 {
        self.t
    }

    pub fn l(&self) -> u16 {
        self.l
    }

    pub fn is_dealer_corrupted(&self) -> bool {
        self.dealer_corruption
    }

    pub fn dealer_corruption(&self) -> u16 {
        if self.is_dealer_corrupted() {
            self.t
        } else {
            0
        }
    }

    pub fn n(&self) -> u16 {
        self.n
    }

    pub fn index(&self) -> u16 {
        self.index
    }

    pub fn is_byz(&self) -> bool {
        self.is_byz
    }

    pub fn dealer(&self) -> u16 {
        self.dealer
    }

    pub fn degree(&self) -> u32 {
        (match self.algo {
            Algo::Badger => self.l(),
            Algo::AvssSimpl => self.t() * 2,
            _ => self.t(),
        }) as u32
    }

    pub fn is_dual_threshold(&self) -> bool {
        self.l > self.t
    }

    pub fn get_batch_evaluation_domain(&self) -> &BatchEvaluationDomain {
        &self.batch_dom
    }

    pub fn get_evaluation_domain(&self) -> &EvaluationDomain {
        &self.dom
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    pub fn index_secret(&self, i: usize) -> usize {
        self.n() as usize + i
    }

    pub fn domain_size(&self) -> usize {
        self.algo.domain_size(self.n, self.t) as usize
    }

    pub fn m(&self) -> usize {
        (self.n() + self.t() + 1) as usize
    }
}
