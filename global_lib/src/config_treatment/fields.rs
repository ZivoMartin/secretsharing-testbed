use crate::{as_number, messages::Algo, Evaluation, KindEvaluation, Step};
use serde::{Deserialize, Serialize};
use tokio::time::Duration;

use std::fmt::{Display, Error as FmtErr, Formatter};

as_number!(
    usize,
    enum TypeField {
        N,
        DealerCorruption,
        BatchSize,
        TDenom,
        LDenom,
        NbByz,
        T,
        L,
    },
    derive(Debug, Eq, Copy, Clone, PartialEq, Deserialize, Serialize)
);

pub const NB_FIELDS: usize = 6;
pub const TO_DISPLAY: usize = 4;
pub static STATIC_TYPE_FIELD: [&str; NB_FIELDS] =
    ["n", "dealer_corruption", "batch_size", "t", "l", "nb_byz"];

impl Display for TypeField {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtErr> {
        write!(
            f,
            "{}",
            String::from(STATIC_TYPE_FIELD[Into::<usize>::into(*self)])
        )
    }
}

impl From<&str> for TypeField {
    fn from(s: &str) -> TypeField {
        STATIC_TYPE_FIELD
            .iter()
            .position(|elt| elt == &s)
            .unwrap_or_else(|| panic!("unvalid string: {s}"))
            .into()
    }
}

impl TypeField {
    pub fn to_axe_name(&self) -> &'static str {
        match self {
            TypeField::N => "Total players (N)",
            TypeField::BatchSize => "Size of the batch",
            TypeField::NbByz => "Number of Byzantin node in the network",
            TypeField::TDenom => "Threshold",
            TypeField::LDenom => "Second Threshold",
            _ => panic!("Field {self} is not allowed on an x axe"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fields {
    fields: Vec<u16>,
    algo: Algo,
    base_latency: Option<Duration>,
    eval: Evaluation,
}

impl Fields {
    pub fn empty() -> Self {
        let mut res = Fields {
            fields: vec![0; STATIC_TYPE_FIELD.len()],
            algo: Algo::default(),
            base_latency: None,
            eval: Evaluation::default(),
        };
        res.set(TypeField::BatchSize, 1);
        res
    }

    pub fn warm_up(n: u16) -> Self {
        Self {
            fields: vec![n, 0, 3, 33, 0, 0],
            algo: Algo::AvssSimpl,
            base_latency: None,
            eval: Evaluation::Latency(Step::Sharing),
        }
    }

    pub fn has_base(&self) -> bool {
        self.base_latency.is_some()
    }

    pub fn get_base(&self) -> Duration {
        self.base_latency.unwrap()
    }

    pub fn set_base(&mut self, b: Duration) {
        self.base_latency = Some(b);
    }

    pub fn set_eval(&mut self, eval: Evaluation) {
        self.eval = eval
    }

    pub fn label_format(kind: &str, val: u16) -> String {
        format!(
            "{kind}: {}",
            if kind == "dealer_corruption" {
                (val == 1).to_string()
            } else {
                format!(
                    "{val}{}",
                    match kind {
                        "t" | "l" => " %",
                        _ => "",
                    }
                )
            }
        )
        .replace("_", " ")
    }

    pub fn get_labels(&self, ignore: Vec<String>) -> String {
        let res = self
            .fields
            .iter()
            .zip(STATIC_TYPE_FIELD.iter())
            .take(TO_DISPLAY)
            .filter(|(_, kind)| !ignore.contains(&kind.to_string()))
            .map(|(val, kind)| Self::label_format(kind, *val) + ", ")
            .collect::<String>();
        res[..res.len() - 2].to_string()
    }

    pub fn set_algo(&mut self, algo: Algo) {
        self.algo = algo
    }

    pub fn set_step(&mut self, step: Step) {
        self.eval.change_step(step);
    }

    pub fn get(&self, field: TypeField) -> u16 {
        self.fields[Into::<usize>::into(field)]
    }

    pub fn set_l(&mut self, l: u16) {
        self.set(TypeField::LDenom, l)
    }

    pub fn set(&mut self, field: TypeField, val: u16) {
        self.fields[Into::<usize>::into(field)] = val;
    }

    pub fn dealer_corruption(&self) -> u16 {
        self.get(TypeField::DealerCorruption)
    }

    pub fn n(&self) -> u16 {
        self.get(TypeField::N)
    }

    fn adjust(&self, v: u16, denom: TypeField) -> u16 {
        (v as f32 * (self.get(denom) as f32 / 100.0)) as u16
    }

    pub fn t(&self) -> u16 {
        self.adjust(self.n(), TypeField::TDenom)
    }

    pub fn l(&self) -> u16 {
        let t = self.t();
        t + self.adjust(t, TypeField::LDenom)
    }

    pub fn nb_byz(&self) -> u16 {
        self.get(TypeField::NbByz)
    }

    pub fn batch_size(&self) -> u16 {
        self.get(TypeField::BatchSize)
    }

    pub fn algo(&self) -> Algo {
        self.algo
    }

    pub fn step(&self) -> Step {
        self.eval.get_step()
    }

    pub fn eval_kind(&self) -> KindEvaluation {
        self.eval.get_kind()
    }

    pub fn eval(&self) -> Evaluation {
        self.eval
    }
}

impl Default for Fields {
    fn default() -> Self {
        Fields::empty()
    }
}
