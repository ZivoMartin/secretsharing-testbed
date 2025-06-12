use anyhow::{bail, ensure, Context};

use super::result_fields::Curve;
use crate::settings::WITH_TITLE;
use std::{
    fs::{remove_file, File},
    io::{BufWriter, Write},
    process::Command,
};

const PLOT_SETUP: &str = "set terminal pngcairo\n";
const GRID_SETUP: &str = "
set grid 
show grid\n";

/// This struct is the argument for the curve plotting functions.
#[derive(Debug)]
pub struct PlotCurve {
    /// Path to the output file where the curve will be saved.
    pub output_file: String,
    /// Title of the curve.
    pub title: String,
    /// Data for the curves to display on the Y-axis.
    pub curves: Vec<Curve>,
    /// Data to display on the X-axis.
    pub x_axe: Vec<Curve>,
    /// Name of the curve, used as a legend.
    pub curves_name: Vec<String>,
    /// color of the curve.
    pub curves_colors: Vec<String>,
    /// Label for the Y-axis.
    pub y_axe_name: String,
    /// Label for the X-axis.
    pub x_axe_name: String,
    /// A list of labels to annotate the curve.
    pub labels: Vec<String>,
}

impl PlotCurve {
    pub fn merge(self, other: Self, output_file: String) -> PlotCurve {
        PlotCurve {
            output_file,
            title: self.title,
            curves: self
                .curves
                .into_iter()
                .chain(other.curves)
                .collect::<Vec<_>>(),
            x_axe: self
                .x_axe
                .into_iter()
                .chain(other.x_axe)
                .collect::<Vec<_>>(),
            curves_name: self
                .curves_name
                .into_iter()
                .chain(other.curves_name)
                .collect::<Vec<_>>(),

            curves_colors: self
                .curves_colors
                .into_iter()
                .chain(other.curves_colors)
                .collect::<Vec<String>>(),

            y_axe_name: self.y_axe_name,
            x_axe_name: self.x_axe_name,
            labels: self.labels,
        }
    }
}

/// This struct is the argument for the single bar chart plotting function.
pub struct PlotChart<'a> {
    /// Path to the output file where the chart will be saved.
    pub output_file: &'a str,
    /// Title of the chart.
    pub title: &'a str,
    /// Value to represent the height of the single bar (Y-axis).
    pub bar_values: &'a [u64],
    /// Label for the Y-axis.
    pub y_axe_name: &'a str,
    /// Label for the X-axis (can be the label for the single bar).
    pub x_axe_name: &'a str,
    /// Label to be displayed on the X-axis below the bar.
    pub bar_labels: &'a [String],
    /// A list of labels to annotate the curve.
    pub labels: &'a [String],
}

pub fn plot_bar_chart(args: PlotChart) -> anyhow::Result<()> {
    let data_file_path = "data_single_bar_temp.txt";
    {
        let data_file = File::create(data_file_path).context("Creating data file")?;
        let mut data_writer = BufWriter::new(data_file);
        for (label, val) in args.bar_labels.iter().zip(args.bar_values.iter()) {
            writeln!(data_writer, "\"{label}\" {val}").context("writing label in data file")?
        }
    }

    let script_file_path = "script_single_bar_temp.gp";
    {
        let max = *args.bar_values.iter().max().context("Searching maximum")? as f32;
        let min = *args.bar_values.iter().min().context("Searching minumum")? as f32;
        let script_file = File::create(script_file_path).context("Creating script file")?;
        let mut script_writer = BufWriter::new(script_file);
        let mut content = format!(
            "{PLOT_SETUP}
             set title '{}'\n\
             set output '{}'\n\
             set ylabel '{}'\n\
             set xlabel '{}'\n\
             set yrange [0:{}]\n\
             set style data histogram\n\
             set style histogram cluster gap 1\n\
             set style fill solid border -1\n",
            args.title,
            args.output_file,
            args.y_axe_name,
            args.x_axe_name,
            max * (1.1 + if max - min < 500.0 { 0.6 } else { 0.0 })
        ); // Sets up the bar chart for a single bar

        label_plot(&mut content, args.labels);

        content.push_str(&format!(
            "plot '{data_file_path}' using 2:xtic(1) with histogram title 'Latency'\n",
        ));

        write!(script_writer, "{}", content).context("Writing content in the script file")?;
    }
    let status = Command::new("gnuplot")
        .arg(script_file_path)
        .status()
        .context("Executing gnuplot")?;
    if !status.success() {
        bail!("Failed to execute the gnuplot script");
    }

    remove_file(data_file_path).context("Removing data file")?;
    remove_file(script_file_path).context("Removing script file")?;

    Ok(())
}

/// This function generates a line plot using gnuplot from the provided `PlotCurve` data.
/// The process consists of three steps:
/// 1. Writing data points to a temporary file.
/// 2. Creating a gnuplot script that uses the temporary data file to generate the plot.
/// 3. Executing the gnuplot script to create the PNG output.
pub fn plot_curve(args: PlotCurve) -> anyhow::Result<()> {
    if args.curves.iter().any(|c| c.len() <= 1) {
        bail!("Contains an empty curve");
    }
    ensure!(
        args.x_axe.len() == args.curves.len(),
        "test whether there are as many x-axes as there are curves"
    );
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (u64::MAX, u64::MAX, 0, 0);

    fn data_file_path(i: usize) -> String {
        format!("data_file_{i}.txt")
    }

    {
        for (i, (x_axe, y_axe)) in args.x_axe.iter().zip(args.curves.iter()).enumerate() {
            let data_file = File::create(data_file_path(i)).context("Create temp file")?;
            let mut data_writer = BufWriter::new(data_file);

            for (x, y) in x_axe.iter().zip(y_axe.iter()) {
                writeln!(data_writer, "{x} {y}").context("Write data in temp file")?;
                if *x > max_x {
                    max_x = *x
                }
                if *x < min_x {
                    min_x = *x
                }
                if *y > max_y {
                    max_y = *y
                }
                if *y < min_y {
                    min_y = *y
                }
            }
        }
    }

    let script_file_path = "script_temp.gp";
    {
        let max = max_y as f32;
        let min = min_y as f32;

        let script_file = File::create(script_file_path).context("Create script file")?;
        let mut script_writer = BufWriter::new(script_file);
        let mut content = format!(
            "
{PLOT_SETUP}
{GRID_SETUP}
{}
set output '{}'\n\
set ylabel '{}'\n\
set xlabel '{}'\n\
set xrange [{}:{}]\n\
set yrange [0:{}]\n",
            if WITH_TITLE {
                format!("set title '{}'\n", args.title)
            } else {
                String::new()
            },
            args.output_file,
            args.y_axe_name,
            args.x_axe_name,
            min_x,
            max_x,
            max * (1.1 + if max - min < 500.0 { 0.4 } else { 0.0 })
        ); // Creates the plot with the specified title and axis labels

        label_plot(&mut content, &args.labels);
        content.push_str("plot ");
        ensure!(
            args.curves_name.len() == args.curves.len(),
            "{}",
            format!(
                "{} curve for {} name",
                args.curves_name.len(),
                args.curves.len()
            )
        );
        for (i, (result_field, color)) in args
            .curves_name
            .iter()
            .zip(args.curves_colors.iter())
            .enumerate()
        {
            if i > 0 {
                content.push_str(", ");
            }
            content.push_str(&format!(
                "'{}' using 1:2 title '{result_field}' w lp pt 7 ps 1 lc rgb \"{color}\"",
                data_file_path(i)
            ));
        }
        write!(script_writer, "{}", content).context("Writing content in the script file")?;
    }

    let status = Command::new("gnuplot")
        .arg(script_file_path)
        .status()
        .context("Executing gnuplot")?;
    if !status.success() {
        bail!("Failed to execute the gnuplot script");
    }

    for i in 0..args.curves.len() {
        remove_file(data_file_path(i)).context("Removing data file")?;
    }
    remove_file(script_file_path).context("Removing script file")?;
    Ok(())
}

fn label_plot(content: &mut String, labels: &[String]) {
    for (i, label) in labels.iter().enumerate() {
        content.push_str(&format!(
            "set label '{label}' at graph 1.2, {} tc rgb 'skyblue' font 'Arial,36' front\n",
            0.9 - i as f32 * 0.2
        ));
    }
}
