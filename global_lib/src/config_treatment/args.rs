use super::{
    data_type::DataType,
    fields::{Fields, TypeField},
    plot::plot_curve,
    result_fields::{DebitCurves, ResultDuration},
    subargs::SubArgs,
    utils::{
        extract_serde_arr, extract_serde_obj, extract_serde_string, serde_n_to_u16,
        serde_n_to_usize, JsonMap, JsonValue,
    },
};
use crate::{
    config_treatment::plot::PlotCurve, dec, enc, messages::Algo, settings::WARM_UP, Evaluation,
    KindEvaluation, Step,
};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::from_str;
use std::{
    collections::HashSet,
    fs::{read_to_string, OpenOptions},
    io::{Read, Write},
    process::Command,
    str::FromStr,
};

const WARM_UP_COUNT: usize = 10;

/// General struct that allow to load a config file
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Args {
    /// Differents states of the config
    args: Vec<SubArgs>,
    /// Current arg index
    current_arg: usize,
    /// Asked result type in the config file, can contain details median and average
    data_result: HashSet<DataType>,
    /// Output path
    output: String,
    /// The number of warming up operation
    warm_up_counter: usize,
    /// The warm_up config
    warm_up: Fields,
}

impl Args {
    /// Initialize the configration environnement.
    fn init(&mut self) {
        assert!(!self.is_empty(), "The config file doesn't have arguments");
        self.warm_up_counter = WARM_UP_COUNT;
        self.current_arg = self
            .args
            .iter()
            .position(|s| s.has_latency())
            .unwrap_or_else(|| {
                self.args
                    .iter()
                    .position(|s| s.has_debit())
                    .expect("Please don't start empty config")
            });
        self.warm_up = Fields::warm_up(self.get_maximum_network_size());
        self.set_eval(if self.current_arg().unwrap().has_latency() {
            KindEvaluation::Latency
        } else {
            KindEvaluation::Debit
        })
        .unwrap();
    }

    /// Modifie the current evaluation of the configuration. Return an error if the configuration is over.
    pub fn set_eval(&mut self, eval: KindEvaluation) -> Result<(), String> {
        let current_arg = match self.current_arg_mut() {
            Some(arg) => arg,
            None => {
                return Err(format!(
                    "Failed to set the evaluation as {eval:?}, the config is over."
                ))
            }
        };
        let eval = eval.to_eval(if current_arg.has_sharing() {
            Step::Sharing
        } else {
            Step::Reconstruct
        });
        current_arg.set_eval(eval);
        Ok(())
    }

    /// Returns the number of node needed to handle the entire config
    pub fn get_maximum_network_size(&self) -> u16 {
        self.args
            .iter()
            .map(|s| s.get_maximum_network_size())
            .max()
            .unwrap()
    }

    /// Return the duration of the debit evaluation for the current args. Fails if the config is over but return 0 if the debit isn't evaluated on this state of the config
    pub fn debit_duration(&self) -> Result<usize, String> {
        match self.current_arg(){
            Some(arg) => Ok(arg.debit()),
            None => Err(format!("Failed to get the debit duration because the config is over. Current arg: {}, config size: {}", self.current_arg, self.len()))
        }
    }

    /// Return the curent step evaluated from the current evaluation.
    pub fn step(&self) -> anyhow::Result<Step> {
        Ok(self
            .current_arg()
            .context("Failed to get the current step because the config is over")?
            .get_step())
    }

    /// Return the curent evaluation. This function isn't dependant of the current state of the config
    pub fn eval(&self) -> Evaluation {
        self.current_arg()
            .expect("Failed to get the current step because the config is over")
            .get_eval()
    }

    /// Return the curent evaluation. This function isn't dependant of the current state of the config
    pub fn get_kind_eval(&self) -> KindEvaluation {
        self.eval().get_kind()
    }

    /// Returns true if the config is in warm up stage
    pub fn warm_up_is_over(&self) -> bool {
        self.warm_up_counter == 0
    }

    /// Call this function after a warm_up operation
    pub fn warm_up_evolve(&mut self) {
        self.warm_up_counter -= 1
    }

    pub fn warm_up_n(&self) -> usize {
        WARM_UP_COUNT
    }

    /// Returns the currents fields of the configurations, if the configuration is over the function returns nothing
    pub fn get_fields(&self) -> Option<&Fields> {
        Some(if WARM_UP && !self.warm_up_is_over() {
            &self.warm_up
        } else {
            self.current_arg()?.fields()
        })
    }

    /// Returns the currents fields of the configurations, if the configuration is over the function returns nothing
    pub fn get_fields_mut(&mut self) -> Option<&mut Fields> {
        Some(if WARM_UP && !self.warm_up_is_over() {
            &mut self.warm_up
        } else {
            self.current_arg_mut()?.fields_mut()
        })
    }

    /// Returns the current state of the config by using the current_arg index, this function fails if the config is over i.e current_arg > number of state
    fn current_arg(&self) -> Option<&SubArgs> {
        self.args.get(self.current_arg)
    }

    /// Same as current_arg but returns the state as mutable
    fn current_arg_mut(&mut self) -> Option<&mut SubArgs> {
        self.args.get_mut(self.current_arg)
    }

    /// Returns ture if the config is over
    pub fn is_over(&self) -> bool {
        self.current_arg == self.len()
    }

    /// Simply evolving the debit, takes as parameter the computed curves
    pub fn debit_evolve(&mut self, curves: DebitCurves) -> anyhow::Result<()> {
        assert!(!self.is_over());
        let data_result = self.data_result.clone();
        let output = &self.output.clone();
        if self
            .current_arg_mut()
            .expect("Config is over !")
            .debit_evolve(curves, output, data_result)?
        {
            self.try_to_go_next();
        }
        if self.is_over() {
            self.save()
        }
        Ok(())
    }

    /// Simply evolving the latency, takes as parameter the computed results and call generic evolve with good parameters
    pub fn latency_evolve(&mut self, result: ResultDuration) -> anyhow::Result<()> {
        assert!(!self.is_over());
        let data_result = self.data_result.clone();
        let output = &self.output.clone();
        if self
            .current_arg_mut()
            .expect("Config is over !")
            .latency_evolve(result, output, data_result)?
            && self.try_to_go_next()
        {
            if let Some(i) = self.args.iter().position(|s| s.has_debit()) {
                self.current_arg = i;
                self.set_eval(KindEvaluation::Debit).unwrap();
            }
        }
        if self.is_over() {
            self.save()
        }
        Ok(())
    }

    /// The goal of this function is to finalize the evolution. It tries to catch the next element in the states array that is of the same evaluation type as the one passed as arguments
    fn try_to_go_next(&mut self) -> bool {
        self.current_arg += self.args[self.current_arg + 1..]
            .iter()
            .position(match self.get_kind_eval() {
                KindEvaluation::Debit => |subarg: &SubArgs| subarg.has_debit(),
                KindEvaluation::Latency => |subarg: &SubArgs| subarg.has_latency(),
            })
            .unwrap_or(self.len() - self.current_arg - 1)
            + 1;
        if !self.is_over() {
            println!("New varied field: {}", self.current_arg().unwrap().varied());
        }
        self.is_over()
    }

    /// This function reset the current states, used to recompute from the beggining in case of an error
    pub fn reset(&mut self) -> Result<(), String> {
        match self.current_arg_mut() {
            Some(arg) => {
                arg.full_reset();
                Ok(())
            }
            None => Err(format!(
                "Failed to reset the current argument {}, size of the config: {}",
                self.current_arg,
                self.len()
            )),
        }
    }

    /// Returns the number of state of the config
    pub fn len(&self) -> usize {
        self.args.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns n in the current fields (number of nodes)
    pub fn n(&self) -> Option<u16> {
        Some(self.get_fields()?.n())
    }

    /// Returns t in the current fields (maximum number of byzantine nodes)
    pub fn t(&self) -> Option<u16> {
        Some(self.get_fields()?.t())
    }

    /// Returns l in the current fields (second threshold)
    pub fn l(&self) -> Option<u16> {
        Some(self.get_fields()?.l())
    }

    /// Returns nb_byz in the current fields (number of byzantine node)
    pub fn nb_byz(&self) -> Option<u16> {
        Some(self.get_fields()?.get(TypeField::NbByz))
    }

    /// Returns the number of time the current state has to be evaluated. Return None if the config is over
    pub fn hmt(&self) -> Option<usize> {
        Some(self.current_arg()?.hmt())
    }

    /// This function iterate through the args and apply the modifications necessarry for each of them:
    ///        output => Output file
    ///        result_type => Array of results type, can contains details median and average
    fn handle_args(res: &mut Args, args: &JsonValue) {
        let args = extract_serde_obj(args);
        for (key, value) in args.iter() {
            let key = key as &str;
            match key {
                "output" => res.set_output(extract_serde_string(value).to_string()),
                "result_type" => {
                    for d in extract_serde_arr(value) {
                        res.data_result.insert(
                            DataType::from_str(extract_serde_string(d))
                                .unwrap_or_else(|_| panic!("Unvalid data type: {d}")),
                        );
                    }
                }
                _ => panic!("Unvalid field in args: {key}"),
            }
        }
    }

    /// This function will compute the setup contains in the given JsonMap, fails if the map is invalid. It gives the setup to the state passed in argument
    fn handle_setup(setup: &JsonMap, subarg: &mut SubArgs) {
        let mut variations: Vec<(TypeField, Vec<u16>)> = Vec::new();
        let mut main = None;
        for (key, value) in setup {
            match value {
                JsonValue::Number(n) => subarg.set_field_from_str(key, n.as_u64().unwrap() as u16),
                JsonValue::Array(arr) => match key as &str {
                    "steps" => subarg.set_steps(
                        extract_serde_arr(value)
                            .iter()
                            .map(|step| Step::from(extract_serde_string(step) as &str))
                            .collect(),
                    ),

                    "algos" => subarg.set_algos(
                        extract_serde_arr(value)
                            .iter()
                            .map(|algo| Algo::from(extract_serde_string(algo) as &str))
                            .collect(),
                    ),
                    _ => {
                        variations.push((
                            TypeField::from(key as &str),
                            arr.iter().map(serde_n_to_u16).collect::<Vec<u16>>(),
                        ));
                    }
                },
                JsonValue::String(s) => {
                    assert!(key == "main", "Invalid arg for a state setup: {key}");
                    main = Some(TypeField::from(s as &str))
                }
                _ => panic!("Invalid arg for a state setup: {key}"),
            }
        }
        if let Some(main) = main {
            assert!(variations.iter().any(|(v, _)| *v == main));
            for (t, v) in variations {
                if t == main {
                    subarg.set_variation(t, v)
                } else {
                    subarg.add_subvariation(t, v)
                }
            }
        } else {
            assert!(
                variations.len() == 1,
                "You gave more than one variation without explicting the main."
            );
            let (t, v) = variations.pop().unwrap();
            subarg.set_variation(t, v)
        }
    }

    /// This functions handle the debit arguments:
    ///      duration => The duration of the debit evaluation
    ///      _ => others are simply results asked in the config such as debit_sharing or debit_derived_sharing
    fn handle_debit(val: &JsonMap, subarg: &mut SubArgs) {
        for (key, value) in val.iter() {
            let key = key as &str;
            match key {
                "duration" => subarg.set_debit_duration(serde_n_to_usize(value)),
                _ => panic!("Unvalid parameter for debit: {key}"),
            }
        }
        subarg.set_eval(Evaluation::Debit(Step::Sharing));
    }

    /// This function handle latencu arguments:
    ///     hmt => The number of time the process has to be repeated
    ///     steps => The step that the state will store in memory, can contains latency_sharing, latency_reconstruct,
    fn handle_latency(val: &JsonMap, args: &mut SubArgs) {
        for (key, value) in val.iter() {
            match key as &str {
                "hmt" => args.set_hmt(serde_n_to_usize(value)),
                _ => panic!("Invalid parameter for latency: {key}"),
            }
        }
    }

    /// This function will load a configuration from a file path. If the path or the configuration is invalid the function fails.
    pub fn from_file(path: String) -> Self {
        let mut res = Args::default();
        let content = read_to_string(path).expect("Path invalid");
        let value: JsonValue = from_str(&content).expect("The given json file is invalid");
        let json_args = extract_serde_arr(&value);
        Self::handle_args(&mut res, &json_args[0]);
        for (i, sim) in json_args.iter().skip(1).map(extract_serde_obj).enumerate() {
            let mut subarg = SubArgs::new(i);
            for (key, val) in sim {
                match key as &str {
                    "debit" => Self::handle_debit(extract_serde_obj(val), &mut subarg),
                    "latency" => Self::handle_latency(extract_serde_obj(val), &mut subarg),
                    "output_file" => subarg.set_name(extract_serde_string(val).clone()),
                    "setup" => Self::handle_setup(extract_serde_obj(val), &mut subarg),
                    _ => panic!("Invalid global parameter: {}", key),
                }
            }
            assert!(subarg.has_name());
            res.args.push(subarg);
        }
        res.init();
        res
    }

    /// Returns true if the current state asking reconstruction step
    pub fn has_reconstruct(&self) -> Option<bool> {
        Some(self.current_arg()?.has_reconstruct())
    }

    pub fn algos(&self) -> Vec<Algo> {
        self.current_arg().unwrap().algos()
    }

    /// Return the current algo
    pub fn get_algo(&self) -> Option<Algo> {
        Some(self.get_fields()?.algo())
    }

    /// This function will set the output file of the configuration. So it will create a new repo at the location /config/results/ who has the name of the output, then all the png and details file will be stored here
    fn set_output(&mut self, output_file: String) {
        let path = format!("../configs/results/{output_file}");
        let _ = Command::new("rm").arg("-rf").arg(&path).status();
        let status = Command::new("mkdir").arg(&path).status().unwrap();
        if !status.success() {
            panic!("Failed to create the result repo");
        }
        self.output = path;
    }

    fn save(&self) {
        let path = format!("{}/Save.txt", self.output);
        let mut f = OpenOptions::new()
            .append(false)
            .write(true)
            .truncate(true)
            .create(true)
            .open(path)
            .unwrap();
        f.write_all(&enc!(self)).unwrap();
    }

    pub fn regenerate(p: String) -> anyhow::Result<()> {
        let path = format!("../configs/results/{}/Save.txt", p);
        let mut f = OpenOptions::new().read(true).open(path).unwrap();
        let mut b = Vec::new();
        f.read_to_end(&mut b).unwrap();
        let args: Args = dec!(b);
        for mut s in args.args {
            s.output(
                args.data_result.clone(),
                &format!("../configs/results/{p}/"),
            )?
        }
        Ok(())
    }

    pub fn get_plot_curve_from_details(path: String) -> anyhow::Result<PlotCurve> {
        fn read_arr<T>(s: &str, parser: fn(&str) -> T) -> Vec<T> {
            s.trim_matches(['[', ']']).split(", ").map(parser).collect()
        }

        fn read_arr_arr<T>(s: &str, parser: fn(&str) -> T) -> Vec<Vec<T>> {
            s.trim_matches(['[', ']'])
                .split("|")
                .map(|arr| read_arr(arr, parser))
                .collect()
        }

        let mut file = OpenOptions::new()
            .read(true)
            .open(path)
            .context("Opening data file")?;
        let mut content = String::new();
        file.read_to_string(&mut content)
            .context("Reading file content to string")?;
        println!("{content}");
        let mut lines = content.lines().map(|line| {
            line.split_once(": ")
                .map(|(_, value)| value.to_string())
                .unwrap_or_else(|| panic!("Malformed line: {}", line))
        });

        let output_file = lines.next().context("Missing 'output_file'")?;
        println!("{output_file}");
        let title = lines.next().context("Missing 'title'")?;

        let number_parser = |v: &str| v.parse::<u64>().unwrap();

        let curves: Vec<Vec<u64>> =
            read_arr_arr(&lines.next().expect("Missing 'curves'"), number_parser);
        let x_axe: Vec<Vec<u64>> =
            read_arr_arr(&lines.next().expect("Missing 'x_axe'"), number_parser);

        let s_parser = |v: &str| v.trim_matches('"').to_string();
        let curves_colors: Vec<String> =
            read_arr(&lines.next().context("Missing 'curves_color'")?, s_parser);
        let curves_name = read_arr(&lines.next().context("Missing 'curves_names'")?, s_parser);

        let y_axe_name = lines.next().context("Missing 'y_axe_name'")?;
        let x_axe_name = lines.next().context("Missing 'x_axe_name'")?;

        Ok(PlotCurve {
            title,
            output_file,
            curves,
            x_axe,
            x_axe_name,
            y_axe_name,
            curves_colors,
            curves_name,
            labels: Vec::new(),
        })
    }

    pub fn regenerate_from_details(path: String) -> anyhow::Result<()> {
        let p = Self::get_plot_curve_from_details(path)?;
        println!("{p:?}");
        plot_curve(p)
    }

    pub fn merge(path1: String, path2: String, output: String) -> anyhow::Result<()> {
        let p1 = Self::get_plot_curve_from_details(path1)?;
        let p2 = Self::get_plot_curve_from_details(path2)?;
        let p3 = p1.merge(p2, output);
        plot_curve(p3)
    }
}
