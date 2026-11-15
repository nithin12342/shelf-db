// sheaf_torch.cpp - the native PyTorch bridge (spec Pillar 11).
//
// Two capabilities:
//   1. get_stalk_tensor exposes BCSF stalk memory as a torch tensor with zero
//      copy, via torch::from_blob. The arena keeps ownership.
//   2. SheafCoboundary is an autograd Function. The forward pass applies the
//      sheaf coboundary delta. The backward pass applies the adjoint delta*,
//      so gradients flow directly through the topological operator.
//
// The reference backward uses index_add_ (scatter-add), which is exactly delta*
// for a graph with identity restrictions. The production path calls the SIMD
// kernel adjoint through sheaf_handle_coboundary_adjoint for speed; the math is
// identical. Build with torch.utils.cpp_extension (see setup.py).

#include "sheaf_torch.h"

#include <torch/extension.h>

namespace {

// Zero-copy view of one stalk. The returned tensor aliases arena memory, so the
// SheafHandle must outlive it. No allocation or transposition occurs here.
torch::Tensor get_stalk_tensor(SheafHandle* handle, uint64_t cell_id) {
    float* ptr = sheaf_handle_stalk_ptr(handle, cell_id);
    int64_t dim = sheaf_handle_stalk_dim(handle, cell_id);
    TORCH_CHECK(ptr != nullptr, "stalk cell not present: ", cell_id);
    auto options = torch::TensorOptions().dtype(torch::kFloat32);
    return torch::from_blob(ptr, {dim}, options);
}

// The sheaf coboundary as an autograd Function.
//   forward:  (delta x)_e = x[v_e] - x[u_e]
//   backward: delta* g scatter-adds +g to v_e and -g to u_e
// u and v are int64 index tensors of equal length (one entry per edge lane).
class SheafCoboundary
    : public torch::autograd::Function<SheafCoboundary> {
public:
    static torch::Tensor forward(torch::autograd::AutogradContext* ctx,
                                 torch::Tensor x, torch::Tensor u,
                                 torch::Tensor v) {
        TORCH_CHECK(x.dim() == 1, "x must be a 1-cochain vector");
        TORCH_CHECK(u.sizes() == v.sizes(), "u and v must have equal length");
        ctx->save_for_backward({u, v});
        ctx->saved_data["n"] = x.size(0);
        return x.index_select(0, v) - x.index_select(0, u);
    }

    static torch::autograd::tensor_list backward(
        torch::autograd::AutogradContext* ctx,
        torch::autograd::tensor_list grad_outputs) {
        auto saved = ctx->get_saved_variables();
        auto u = saved[0];
        auto v = saved[1];
        auto g = grad_outputs[0];
        int64_t n = ctx->saved_data["n"].toInt();
        // delta* : accumulate the edge gradient onto the vertex endpoints.
        auto gx = torch::zeros({n}, g.options());
        gx.index_add_(0, v, g);
        gx.index_add_(0, u, -g);
        // No gradient flows through the index tensors u and v.
        return {gx, torch::Tensor(), torch::Tensor()};
    }
};

// Public wrapper that invokes the autograd Function.
torch::Tensor coboundary(torch::Tensor x, torch::Tensor u, torch::Tensor v) {
    return SheafCoboundary::apply(x, u, v);
}

// Dirichlet energy ||delta x||^2. Minimizing it drives x toward a global
// section (delta x = 0), the harmonic solve used by SNN/GNN training.
torch::Tensor dirichlet_energy(torch::Tensor x, torch::Tensor u,
                               torch::Tensor v) {
    auto d = coboundary(x, u, v);
    return (d * d).sum();
}

} // namespace

PYBIND11_MODULE(TORCH_EXTENSION_NAME, m) {
    m.def("get_stalk_tensor", &get_stalk_tensor,
          "Zero-copy view of a BCSF stalk as a float32 tensor");
    m.def("coboundary", &coboundary,
          "Sheaf coboundary delta with an adjoint backward pass");
    m.def("dirichlet_energy", &dirichlet_energy,
          "Dirichlet energy ||delta x||^2 with gradient support");
}