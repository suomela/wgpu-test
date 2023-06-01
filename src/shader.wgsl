@group(0)
@binding(0)
var<storage, read_write> result: array<u32>;

@compute
@workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    var x: u32 = 0u;
    for (var i = 0; i < 1000; i++) {
        x++;
    }
    result[0] = x;
}
