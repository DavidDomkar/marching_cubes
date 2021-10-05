let EDGE_TABLE = 

[[block]]
struct Output {
    data: [[stride(4)]] array<f32>;
};

struct Triangle {
    a: f32;
    b: f32;
    c: f32;
};

[[group(0), binding(0)]]
var<storage> test: [[access(write)]] Output;

[[stage(compute), workgroup_size(8, 8, 8)]]
fn main([[builtin(global_invocation_id)]] global_id: vec3<u32>) {
    test.data[global_id.x] = 5.0;
}

fn polygonise(points: array<vec3<f32>>>) {

}