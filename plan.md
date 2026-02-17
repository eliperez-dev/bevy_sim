# Cloud Improvements Plan

## 1. Visual Variation (Fix "Perfect Spheres")
- [ ] **Tune Noise:**
    - Increase `CLOUD_SCALE` in `consts.rs` (Lower frequency = larger blobs).
    - Increase `CLOUD_DISPLACEMENT` significantly to warp the sphere more.
- [ ] **Randomize Mesh:**
    - Apply a random non-uniform scale to the cloud transform (e.g., stretch it 2x on the X axis) when spawning.

## 2. Fix Spacing (Fix "Grid Look")
- [ ] **Better Randomization:**
    - The current `pseudo_random` might be too uniform.
    - I'll shift the seed significantly for each cloud attempt.
    - Add a `jitter` to the position so they aren't centered in grid cells (though the current logic `(rx - 0.5) * CHUNK_SIZE` should already cover the whole chunk).
- [ ] **Cluster Logic:**
    - Instead of just 1 big cloud, maybe spawn a "cluster" parent with 2-3 child spheres to make complex shapes.

## 3. Fix Culling (Fix "Disappearing")
- [ ] **Recompute AABB:**
    - In `modify_clouds`, after deforming vertices, call `mesh_clone.compute_aabb();`.
    - This ensures the render engine knows the true extent of the large, deformed cloud.
