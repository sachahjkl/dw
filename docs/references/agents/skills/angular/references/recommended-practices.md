# Angular recommandé long terme

- Prefer explicit inputs/outputs. Avoid vague payload bags.
- Prefer dedicated UI type when screen/flow contract diverge.
- Reuse type only if business meaning same.
- Avoid `any`, `unknown as`, opportunistic casts.
- Avoid god components mixing load, mapping, orchestration, render.
- Extract subcomponent on real UI/domain seam, not line-count cleanup.
