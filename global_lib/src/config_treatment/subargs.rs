use super::{
    data_type::DataType,
    fields::{Fields, TypeField},
    result_fields::{Curve, DebitCurves, ResultCurves, ResultCurvesContent, ResultDuration},
    variations::Variation,
};
use crate::{
    config_treatment::plot::{plot_curve, PlotCurve},
    messages::Algo,
    write_in_file, Evaluation, KindEvaluation, Step,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// This struct represents a specific state of the global configuration
#[derive(Default, Debug, Deserialize, Serialize)]
pub struct SubArgs {
    id: usize,
    /// The number of time we have to process in this state to compute the latency
    latency_hmt: usize,
    /// The duration of the timer for the debit evaluation in this state
    debit_duration: usize,
    /// The setup in this state
    fields: Fields,
    /// The fields variation on the state. Is used on each evolution
    variation: Variation,
    /// The debit results of the state
    latency: HashMap<(Algo, Step), Curve>,
    latency_curve: Curve,
    conclusion: Option<ResultCurves>,
    /// The output file for this subarg, is always a png
    output_file: String,
}

impl SubArgs {
    /// Creates an empty state for the given algo
    pub fn new(id: usize) -> Self {
        SubArgs {
            id,
            ..Default::default()
        }
    }

    /// Set the algos of the state
    pub fn set_algos(&mut self, algos: Vec<Algo>) {
        self.fields.set_algo(algos[0]);
        self.variation.set_algos(algos);
    }

    pub fn set_steps(&mut self, steps: Vec<Step>) {
        self.fields.set_step(steps[0]);
        self.variation.set_steps(steps);
    }

    pub fn variation_index(&self) -> usize {
        self.variation.get_variation_index()
    }

    /// Return true the state is reconstructing for the given evaluation
    pub fn has_reconstruct(&self) -> bool {
        self.variation.steps().contains(&Step::Reconstruct)
    }

    /// Return true if the state evaluates the sharing
    pub fn has_sharing(&self) -> bool {
        self.variation.steps().contains(&Step::Sharing)
    }

    /// Returns the setup of the state
    pub fn fields(&self) -> &Fields {
        &self.fields
    }

    pub fn fields_mut(&mut self) -> &mut Fields {
        &mut self.fields
    }

    /// Returns the max value of n in fields, but if the varied field is n then returns the maximum of the variation
    pub fn get_maximum_network_size(&self) -> u16 {
        self.variation
            .get_maximum_network_size()
            .unwrap_or(self.fields.n())
    }

    pub fn full_reset(&mut self) {
        self.variation.reset_full(&mut self.fields)
    }

    /// Change the field associated with the given str field. Panic if the given string is invalid
    pub fn set_field_from_str(&mut self, field: &str, val: u16) {
        self.fields.set(TypeField::from(field), val)
    }

    pub fn get_variation_size(&self) -> usize {
        self.variation.get_variation().len()
    }

    /// Set the variation, ie all the possible value of the varied in the state
    pub fn set_variation(&mut self, varied: TypeField, variation: Vec<u16>) {
        self.fields.set(varied, variation[0]);
        self.variation.set_variation(varied, variation);
    }

    /// Add a subvariation to the state.
    pub fn add_subvariation(&mut self, varied: TypeField, variation: Vec<u16>) {
        self.fields.set(varied, variation[0]);
        self.variation.add_subvariation(varied, variation);
    }

    /// Just set the debit_duration field of the state
    pub fn set_debit_duration(&mut self, d: usize) {
        self.debit_duration = d;
    }

    /// Just set the latency_hmt field of the state
    pub fn set_hmt(&mut self, hmt: usize) {
        self.latency_hmt = hmt;
    }

    pub fn get_eval(&self) -> Evaluation {
        self.fields.eval()
    }

    pub fn get_eval_kind(&self) -> KindEvaluation {
        self.get_eval().get_kind()
    }

    pub fn get_step(&self) -> Step {
        self.fields.step()
    }

    /// Change the evaluation of the fields
    pub fn set_eval(&mut self, eval: Evaluation) {
        self.fields.set_eval(eval)
    }

    /// Just return the latency_hmt field of the state. Returns 0 in the case when the state doesn't evaluate the latency
    pub fn hmt(&self) -> usize {
        self.latency_hmt
    }

    /// This function evolve the state by calling latency debit on his variation. If it returns stop then the state will output. It will also store the curves in itself, thoses curves will be used by the output function later.
    pub fn debit_evolve(
        &mut self,
        curves: DebitCurves,
        output_path: &str,
        data_result: HashSet<DataType>,
    ) -> anyhow::Result<bool> {
        let DebitCurves { datas, latency } = curves;
        println!("{datas:?}");
        println!("{latency:?}");
        self.latency.insert((self.algo(), self.get_step()), latency);
        let mut conclusion = self.variation.evolve(&mut self.fields, datas);
        if let Some(conclusion) = conclusion.take() {
            self.conclusion = Some(conclusion);
            self.output(data_result, output_path)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// This function evolve the state by calling latency evolve on his variation. If it returns stop then the state will output. If it returns a conclusion, then it will store the result and continue its way.
    pub fn latency_evolve(
        &mut self,
        point: ResultDuration,
        output_path: &str,
        data_result: HashSet<DataType>,
    ) -> anyhow::Result<bool> {
        self.latency_curve.push(point);
        if self.latency_curve.len() == self.hmt() {
            let curve: Curve = self.latency_curve.drain(..).collect();
            let mut conclusion = self.variation.evolve(&mut self.fields, curve);
            if let Some(conclusion) = conclusion.take() {
                self.conclusion = Some(conclusion);
                self.output(data_result, output_path)?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn get_output_path(
        &self,
        repo_path: String,
        state: Option<usize>,
        step: Step,
        data_type: Option<DataType>,
    ) -> String {
        repo_path
            + "/"
            + &self.output_file
            + &if self.nb_steps() > 1 {
                format!("_{step:?}")
            } else {
                String::new()
            }
            + &match state {
                Some(state) => format!("_{}", self.variation.get_specific_state(state)),
                None => String::new(),
            }
            + &match data_type {
                Some(data_type) => format!("_{data_type:?}"),
                _ => String::new(),
            }
    }

    /// Returns the times curves
    pub fn latencies(&self, algo: Algo, step: Step) -> &Curve {
        self.latency.get(&(algo, step)).unwrap()
    }

    /// Returns the current algo
    pub fn algos(&self) -> Vec<Algo> {
        self.variation.algos().clone()
    }

    /// Returns the current algo
    pub fn algo(&self) -> Algo {
        self.fields().algo()
    }

    /// Returns the debit_duration
    pub fn debit(&self) -> usize {
        self.debit_duration
    }

    pub fn varied(&self) -> TypeField {
        self.variation.varied()
    }

    /// Return the varied field from the variation as a string
    pub fn varied_str(&self) -> String {
        self.variation.varied_str()
    }

    /// return true if the state evaluates the debit
    pub fn has_debit(&self) -> bool {
        self.debit_duration != 0
    }

    /// return true if the state evaluates the latency
    pub fn has_latency(&self) -> bool {
        self.latency_hmt != 0
    }

    /// Takes in parameter a kind of result and create the debit png associated by peeking the curves in self.result
    fn generate_debit_curve(
        &self,
        output_path: &str,
        curves: ResultCurvesContent,
        details: bool,
    ) -> anyhow::Result<()> {
        for (step, algo_map) in curves {
            for i in 0..self.get_variation_size() {
                let output_file = self.get_output_path(output_path.to_string(), None, step, None);
                let mut curves = Vec::new();
                let mut names = Vec::new();
                let mut latencies = Vec::new();
                let mut colors = Vec::new();
                for ((algo, title, color), algo_curves) in algo_map.iter() {
                    curves.push(algo_curves[i].clone());
                    names.push(title.to_string());
                    colors.push(color.to_string());
                    latencies.push(self.latencies(*algo, step).clone());
                }
                Self::plot(
                    output_file.clone(),
                    format!(
                        "Debit {step:?} with {} = {}",
                        self.varied_str(),
                        self.variation.get_specific_state(i)
                    ),
                    latencies,
                    curves,
                    names,
                    colors,
                    "Latency (ms)".to_string(),
                    "Number of secret output per second".to_string(),
                    if details {
                        Some(format!("{output_file}_details"))
                    } else {
                        None
                    },
                )?;
            }
        }
        println!("Debit curves generated");
        Ok(())
    }

    fn get_ignored_fields(&self) -> Vec<String> {
        let mut res = self.variation.get_subvarieds_as_string_vec();
        res.push(self.varied_str());
        res
    }

    /// Takes in parameter a kind of result and create the latency png associated by peeking the curves in self.result
    fn generate_latency_chart(
        &self,
        output_path: &str,
        result: ResultCurvesContent,
        data_type: &HashSet<DataType>,
        details: bool,
    ) -> anyhow::Result<()> {
        for (step, algo_map) in result.iter() {
            // let bar_labels = &algo_map
            //     .keys()
            //     .map(|a| a.0.to_string())
            //     .collect::<Vec<String>>();
            for dt in data_type.iter().filter(|d| **d != DataType::Details) {
                // if data_type.contains(&DataType::Details) {
                //     for i in 0..self.get_variation_size() {
                //         let bar_values = &algo_map
                //             .values()
                //             .map(|curves| dt.process(*step, &curves[i]).get_val())
                //             .collect::<Vec<ResultDuration>>();
                //         let output_file =
                //             &self.get_output_path(output_path.to_string(), None, *step, None);
                //         plot_bar_chart(PlotChart {
                //             output_file,
                //             title: &format!(
                //                 "Latency {step:?} with {} = {}",
                //                 self.varied_str(),
                //                 self.variation.get_specific_state(i)
                //             ),
                //             bar_values,
                //             y_axe_name: "Latency (ms)",
                //             x_axe_name: &dt.to_string(),
                //             bar_labels,
                //             labels: &[],
                //         });
                //     }
                // }
                let output_file =
                    self.get_output_path(output_path.to_string(), None, *step, Some(*dt))
                        + "_curve";
                let mut curves_name = Vec::new();
                let mut curves_colors = Vec::new();
                let curves = algo_map
                    .iter()
                    .map(|((_, title, color), curves)| {
                        curves_name.push(title.to_string());
                        curves_colors.push(color.to_string());
                        curves
                            .iter()
                            .map(|curve| dt.process(*step, curve).get_val())
                            .collect::<Curve>()
                    })
                    .collect::<Vec<Curve>>();
                let x_axe = vec![
                    self.variation
                        .get_variation()
                        .iter()
                        .map(|v| *v as u64)
                        .collect::<Curve>();
                    curves.len()
                ];
                Self::plot(
                    output_file.clone(),
                    format!(
                        "Latency {step:?}, {}",
                        self.fields.get_labels(self.get_ignored_fields())
                    ),
                    curves.into_iter().collect::<Vec<Curve>>(),
                    x_axe,
                    curves_name,
                    curves_colors,
                    "Latency (ms)".to_string(),
                    self.varied().to_axe_name().to_string(),
                    if details {
                        Some(format!("{output_file}_details"))
                    } else {
                        None
                    },
                )?;
            }
        }
        println!("Latency charts generated");
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn plot(
        output_file: String,
        title: String,
        curves: Vec<Curve>,
        x_axe: Vec<Curve>,
        curves_name: Vec<String>,
        curves_colors: Vec<String>,
        y_axe_name: String,
        x_axe_name: String,
        details: Option<String>,
    ) -> anyhow::Result<()> {
        if let Some(p) = details {
            let curves = curves.iter().map(|v| format!("{v:?}|")).collect::<String>();
            let x_axe = x_axe.iter().map(|v| format!("{v:?}|")).collect::<String>();
            write_in_file(
                &p,
                &format!(
                    "output_file: {output_file}
title: {title},
curves: [{}]
x_axe: [{}]
curves_color: {curves_colors:?}
curves_name: {curves_name:?}
y_axe_name: {y_axe_name}
x_axe_name: {x_axe_name}",
                    &curves[..curves.len() - 1],
                    &x_axe[..x_axe.len() - 1]
                ),
            );
        }
        plot_curve(PlotCurve {
            output_file,
            title,
            curves,
            x_axe,
            curves_name,
            curves_colors,
            y_axe_name,
            x_axe_name,
            labels: Vec::new(),
        })
    }

    /// Take a set of results that can contains details median or average and a path to repo. Will generates the results png of the state and store them in the output repo
    pub fn output(
        &mut self,
        data_result: HashSet<DataType>,
        output_path: &str,
    ) -> anyhow::Result<()> {
        let mut result = self.conclusion.as_ref().unwrap().clone();
        let curves = result.curves();
        let details = data_result.contains(&DataType::Details);
        if details {
            self.save_details(output_path, &curves)
        }
        match self.get_eval_kind() {
            KindEvaluation::Debit => self.generate_debit_curve(output_path, curves, details)?,
            KindEvaluation::Latency => {
                self.generate_latency_chart(output_path, curves, &data_result, details)?
            }
        }
        self.variation.reset_full(&mut self.fields);
        Ok(())
    }

    /// This function create and returns a details string from his result.
    fn create_details_string(&self, curves: &ResultCurvesContent) -> String {
        let mut details_string = String::new();
        curves.iter().for_each(|(step, algo_map)| {
            algo_map.iter().for_each(|((_, algo, _), curves)| {
                curves.iter().enumerate().for_each(|(i, curve)| {
                    details_string += &format!(
                        "Algo {}: {}, with {} = {}\n",
                        algo,
                        DataType::Details.process(*step, curve).get_details(),
                        self.varied(),
                        self.variation.get_specific_state(i)
                    );
                })
            })
        });
        details_string
    }

    /// This function format the state's data and return it too create the details string.
    pub fn get_details_string(&self, curves: &ResultCurvesContent) -> String {
        let details_string = self.create_details_string(curves);
        format!(
            "
---------------------------------------------------------
Setup:
n: {},
t: {},
l: {}
nb_byz: {},
hmt (latency): {},
debit duration: {},
variation on {} : {:?}

Results:
{}
---------------------------------------------------------
",
            self.fields.n(),
            self.fields.t(),
            self.fields.l(),
            self.fields.nb_byz(),
            self.latency_hmt,
            self.debit_duration,
            self.varied(),
            self.variation.get_variation(),
            details_string,
        )
    }

    /// This functions format the details_string, then open the details file which is /config/results/{output_path}/details and store the details in it
    fn save_details(&self, output_path: &str, curves: &ResultCurvesContent) {
        let details = self.get_details_string(curves);
        write_in_file(
            &format!(
                "{}_recap",
                &self.get_output_path(output_path.to_string(), None, Step::Sharing, None)
            ),
            &details,
        );
    }

    pub fn set_name(&mut self, mut output_file: String) {
        if !output_file.ends_with(".png") {
            output_file.push_str(".png")
        }
        self.output_file = output_file
    }

    pub fn has_name(&self) -> bool {
        !self.output_file.is_empty()
    }

    pub fn nb_steps(&self) -> usize {
        self.variation.steps().len()
    }
}
