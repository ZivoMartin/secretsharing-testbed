use super::{
    fields::{Fields, TypeField},
    result_fields::{AlgoKey, Curve, ResultCurves},
};
use crate::{messages::Algo, Step};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Debug, Deserialize, Serialize)]
struct VariationData {
    steps: Vec<Step>,
    algos: Vec<Algo>,
    field: TypeField,
    subvariations: Vec<(TypeField, Vec<u16>)>,
    main_variation: Vec<u16>,
    hmt: usize,
}

impl VariationData {
    pub fn empty() -> VariationData {
        VariationData {
            algos: Algo::all(),
            field: TypeField::N,
            main_variation: Vec::new(),
            steps: Step::all(),
            hmt: 1,
            subvariations: Vec::new(),
        }
    }

    fn get_maximum_network_size(&self) -> Option<u16> {
        if self.field == TypeField::N {
            Some(*self.main_variation.iter().max().unwrap_or(&0))
        } else {
            self.subvariations
                .iter()
                .find(|(t, _)| *t == TypeField::N)
                .map(|(_, v)| *v.iter().max().unwrap_or(&0))
        }
    }

    fn set_algos(&mut self, algos: Vec<Algo>) {
        self.algos = algos;
    }

    fn add_subvariation(&mut self, varied: TypeField, variation: Vec<u16>) {
        self.subvariations.push((varied, variation))
    }

    fn set_variation(&mut self, varied: TypeField, variation: Vec<u16>) {
        self.field = varied;
        self.main_variation = variation;
    }
}

#[derive(Deserialize, Debug, Serialize)]
pub struct Variation {
    data: VariationData,
    variation_index: usize,
    algo_variation_index: usize,
    step_variation_index: usize,
    subvariation_index: usize,
    subvaried_index: usize,
    conclusion: Option<ResultCurves>,
}

impl Default for Variation {
    fn default() -> Self {
        Self::new()
    }
}

impl Variation {
    pub fn new() -> Variation {
        Variation {
            data: VariationData::empty(),
            variation_index: 0,
            algo_variation_index: 0,
            step_variation_index: 0,
            subvariation_index: 0,
            subvaried_index: 0,
            conclusion: Some(ResultCurves::new()),
        }
    }

    pub fn get_variation_index(&self) -> usize {
        self.variation_index
    }

    pub fn get_maximum_network_size(&self) -> Option<u16> {
        self.data.get_maximum_network_size()
    }

    pub fn add_subvariation(&mut self, varied: TypeField, variation: Vec<u16>) {
        self.data.add_subvariation(varied, variation);
    }

    pub fn set_variation(&mut self, varied: TypeField, variation: Vec<u16>) {
        self.data.set_variation(varied, variation);
    }

    pub fn set_algos(&mut self, algos: Vec<Algo>) {
        self.data.set_algos(algos)
    }

    pub fn set_steps(&mut self, steps: Vec<Step>) {
        self.data.steps = steps
    }

    pub fn reset_full(&mut self, fields: &mut Fields) {
        self.algo_variation_index = 0;
        self.step_variation_index = 0;
        self.variation_index = 0;
        fields.set_algo(self.algo());
        fields.set_step(self.step());
        self.actualise_fields(fields);
    }

    fn actualise_fields(&self, fields: &mut Fields) {
        fields.set(
            TypeField::from(&self.varied_str() as &str),
            self.current_variation(),
        );
        for (t, v) in &self.data.subvariations {
            if *t == self.current_subvaried() {
                fields.set(*t, self.current_subvariation_state());
            } else {
                fields.set(*t, v[0]);
            }
        }
        fields.set_algo(self.algo());
        fields.set_step(self.step());
        if self.data.algos.contains(&Algo::Bingo) {
            fields.set(TypeField::BatchSize, fields.get(TypeField::N) / 3)
        }
    }

    fn next(&mut self) -> bool {
        self.inc_variation_index()
            && self.inc_subvariation_index()
            && self.inc_subvaried_index()
            && self.inc_algo_index()
            && self.inc_step_index()
    }

    fn inc_variation_index(&mut self) -> bool {
        self.variation_index += 1;
        let over = self.variation_index == self.data.main_variation.len();
        if over {
            self.variation_index = 0;
        } else {
            println!("Advancing variation, new: {}", self.current_variation())
        }
        over
    }

    fn inc_subvariation_index(&mut self) -> bool {
        if !self.has_subvariations() {
            return true;
        }
        self.subvariation_index += 1;
        let over = self.subvariation_index == self.get_current_subvariation().len();
        if over {
            self.subvariation_index = 0;
        } else {
            println!(
                "Advancing subvariation, new: {}",
                self.current_subvariation_state()
            )
        }
        over
    }

    fn inc_subvaried_index(&mut self) -> bool {
        if !self.has_subvariations() {
            return true;
        }
        self.subvaried_index += 1;
        let over = self.subvaried_index == self.data.subvariations.len();
        if over {
            self.subvaried_index = 0;
        } else {
            println!(
                "Advancing subvaried, new: {}",
                self.data.subvariations[self.subvaried_index].0
            )
        }
        over
    }

    fn inc_algo_index(&mut self) -> bool {
        self.algo_variation_index += 1;
        while self.algo_variation_index < self.algos().len() && !self.algo().support(self.step()) {
            self.algo_variation_index += 1;
        }
        let over = self.algo_variation_index == self.algos().len();
        if over {
            self.algo_variation_index = 0;
        } else {
            println!("Switching algo: {}", self.algo())
        }
        over
    }

    fn inc_step_index(&mut self) -> bool {
        self.step_variation_index += 1;
        let over = self.step_variation_index == self.steps().len();
        if over {
            self.step_variation_index = 0;
        } else {
            println!("Switching step: {:?}", self.step())
        }

        over || (!self.algo().support(self.step())
            && self.inc_algo_index()
            && self.inc_step_index())
    }

    pub fn evolve(&mut self, args: &mut Fields, result: Curve) -> Option<ResultCurves> {
        self.insert_result(self.step(), self.algo(), result);
        if self.next() {
            Some(self.extract_conclusion())
        } else {
            self.actualise_fields(args);
            None
        }
    }

    fn extract_conclusion(&mut self) -> ResultCurves {
        self.conclusion.take().unwrap()
    }

    fn current_algo_key(&self, algo: Algo) -> AlgoKey {
        (
            algo,
            if self.has_subvariations() {
                format!(
                    "{} with {} = {}",
                    algo,
                    self.current_subvaried(),
                    self.current_subvariation_state()
                )
                .replace("_", " ")
            } else {
                algo.to_string()
            },
            algo.curve_color().to_string(),
        )
    }

    fn insert_result(&mut self, step: Step, algo: Algo, result: Curve) {
        assert!(algo.support(step));
        let algo_key = self.current_algo_key(algo);
        self.conclusion
            .as_mut()
            .unwrap()
            .insert(result, step, algo_key);
    }

    fn has_subvariations(&self) -> bool {
        !self.data.subvariations.is_empty()
    }

    fn get_current_subvariation(&self) -> &Vec<u16> {
        &self.data.subvariations[self.subvaried_index].1
    }

    pub fn get_formated_variation(&self, n: u16) -> Vec<u32> {
        if self.data.field == TypeField::TDenom {
            self.data
                .main_variation
                .iter()
                .map(|t| (n as f32 * (*t as f32 / 100.0)) as u32)
                .collect()
        } else {
            self.data.main_variation.iter().map(|v| *v as u32).collect()
        }
    }

    pub fn current_subvaried(&self) -> TypeField {
        self.data.subvariations[self.subvaried_index].0
    }

    pub fn current_subvariation_state(&self) -> u16 {
        self.get_current_subvariation()[self.subvariation_index]
    }

    pub fn varied_str(&self) -> String {
        self.data.field.to_string()
    }

    pub fn varied(&self) -> TypeField {
        self.data.field
    }

    pub fn get_variation(&self) -> &Vec<u16> {
        &self.data.main_variation
    }

    pub fn get_specific_state(&self, i: usize) -> u16 {
        self.get_variation()[i]
    }

    pub fn get_step_index(&self) -> usize {
        self.step_variation_index
    }

    pub fn steps(&self) -> &Vec<Step> {
        &self.data.steps
    }

    pub fn step(&self) -> Step {
        self.data.steps[self.step_variation_index]
    }

    pub fn current_variation(&self) -> u16 {
        self.data.main_variation[self.variation_index]
    }

    pub fn get_algo_index(&self) -> usize {
        self.algo_variation_index
    }

    pub fn algos(&self) -> &Vec<Algo> {
        &self.data.algos
    }

    pub fn algos_as_str(&self, step: Step) -> Vec<String> {
        self.data
            .algos
            .iter()
            .filter(|a| a.support(step))
            .map(|a| a.to_string())
            .collect()
    }

    pub fn get_subvarieds_as_string_vec(&self) -> Vec<String> {
        self.data
            .subvariations
            .iter()
            .map(|(t, _)| t.to_string())
            .collect()
    }

    pub fn algo(&self) -> Algo {
        self.data.algos[self.algo_variation_index]
    }
}
