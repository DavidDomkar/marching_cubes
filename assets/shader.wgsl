[[block]]
struct Lulw {
    data: [[stride(4)]] array<u32>;
};

[[group(0), binding(0)]]
var<storage> test: [[access(write)]] Lulw;

[[stage(compute), workgroup_size(2)]]
fn main([[builtin(global_invocation_id)]] global_id: vec3<u32>) {
    test.data[global_id.x] = 5u;
}