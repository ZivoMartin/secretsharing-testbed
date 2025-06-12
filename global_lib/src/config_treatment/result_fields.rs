pub type ResultDuration = u64;
use crate::{messages::Algo, settings::DEBIT_CURVE_NB_POINT, Step};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type AlgoKey = (Algo, String, String); // Algo, CurveColor
pub type ResultCurvesContent = HashMap<Step, HashMap<AlgoKey, Vec<Curve>>>;

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
pub struct ResultCurves {
    curves: Option<ResultCurvesContent>,
}

impl ResultCurves {
    pub fn new() -> Self {
        Self {
            curves: Some(ResultCurvesContent::new()),
        }
    }

    pub fn curves(&mut self) -> ResultCurvesContent {
        let res = self.curves.take().unwrap();
        self.curves = Some(ResultCurvesContent::new());
        res
    }

    pub fn insert(&mut self, curve: Curve, step: Step, algo_key: AlgoKey) {
        let curves = self.curves.as_mut().unwrap();
        if let Some(algo_map) = curves.get_mut(&step) {
            if let Some(curves) = algo_map.get_mut(&algo_key) {
                curves.push(curve);
            } else {
                algo_map.insert(algo_key, vec![curve]);
            }
        } else {
            curves.insert(step, HashMap::new());
            self.insert(curve, step, algo_key);
        }
    }
}

pub type Curve = Vec<ResultDuration>;

pub struct DebitCurves {
    pub datas: Curve,
    pub latency: Curve,
}

impl Default for DebitCurves {
    fn default() -> Self {
        Self::new()
    }
}

impl DebitCurves {
    pub fn new() -> Self {
        Self {
            datas: Vec::with_capacity(DEBIT_CURVE_NB_POINT),
            latency: Vec::with_capacity(DEBIT_CURVE_NB_POINT),
        }
    }

    pub fn compute_derived(curve: &Curve) -> Curve {
        let mut curve: Curve = curve
            .iter()
            .zip(curve.iter().skip(1))
            .map(|(prev, x)| x - prev)
            .collect();
        curve.insert(0, 0);
        curve
    }

    pub fn push(&mut self, x: u64, l: u64) {
        self.datas.push(x);
        self.latency.push(l);
    }

    pub fn last(&self) -> u64 {
        *self.datas.last().unwrap()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.datas.len()
    }
}
