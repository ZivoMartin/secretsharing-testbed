pub type JsonValue = serde_json::Value;
pub type JsonMap = serde_json::Map<String, JsonValue>;
use serde_json::Number;

use super::result_fields::{Curve, ResultDuration};

pub fn adjust_curve(x_curve: &mut Curve, y_curve: &mut Curve) {
    assert!(x_curve.len() == y_curve.len());
    let lim = (*x_curve.last().expect("x is empty") as f32 * 0.05) as ResultDuration;
    if x_curve[0] != 0 {
        x_curve.insert(0, 0);
        y_curve.insert(0, 0);
    }
    let mut new_x = vec![0];
    let mut new_y = vec![y_curve[0]];
    for (((x, y), prev_x), prev_y) in x_curve
        .iter()
        .skip(1)
        .zip(y_curve.iter().skip(1))
        .zip(x_curve.iter())
        .zip(y_curve.iter())
    {
        let dx = x - prev_x;
        let dy = y - prev_y;
        if dx > lim {
            let step = dy / dx;
            for added_x in *prev_x..*x {
                new_x.push(added_x);
                new_y.push(y + (added_x - prev_x) * step);
            }
        }
        new_x.push(*x);
        new_y.push(*y);
    }
    *x_curve = new_x;
    *y_curve = new_y;
}

pub fn extract_serde_obj(v: &JsonValue) -> &JsonMap {
    match v {
        JsonValue::Object(m) => m,
        _ => panic!("Failed to extract Object"),
    }
}

pub fn extract_serde_obj_mut(v: &mut JsonValue) -> &mut JsonMap {
    match v {
        JsonValue::Object(m) => m,
        _ => panic!("Failed to extract Object"),
    }
}

pub fn extract_serde_string(v: &JsonValue) -> &String {
    match v {
        JsonValue::String(s) => s,
        _ => panic!("Failed to extract String"),
    }
}

pub fn extract_serde_arr(v: &JsonValue) -> &Vec<JsonValue> {
    match v {
        JsonValue::Array(arr) => arr,
        _ => panic!("Failed to extract Array"),
    }
}

pub fn serde_n_to_u16(n: &JsonValue) -> u16 {
    if let JsonValue::Number(n) = n {
        n.as_u64().unwrap() as u16
    } else {
        panic!("Given array is not fully a number array")
    }
}

pub fn serde_n_to_usize(n: &JsonValue) -> usize {
    if let JsonValue::Number(n) = n {
        n.as_u64().unwrap() as usize
    } else {
        panic!("Given array is not fully a number array")
    }
}

pub fn vec_to_serde_arr<T: Into<u64> + Copy>(vec: &[T]) -> JsonValue {
    JsonValue::Array(
        vec.iter()
            .map(|x| JsonValue::Number(Number::from(Into::<u64>::into(*x))))
            .collect(),
    )
}
