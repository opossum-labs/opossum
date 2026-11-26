# Fluence uncertainty

This page derives the error estimates a [fluence detector](../nodes/fluence_detector.md) reports with its property `error estimates` switched on, for the Voronoi estimator: the map `Fluence uncertainty` of how far the fluence may lie from the map between the rays, and `Peak fluence uncertainty`, how far the peak of the fluence may lie above the peak of the map. It states exactly what they promise and what they do not. [Fluence calculations](../../concepts/fluence.md#uncertainty) describes them without the mathematics.

The map follows the ray tubes the source spans (see [Fluence from ray tubes](../../concepts/fluence.md#fluence-from-ray-tubes)): it knows the fluence at the corners of the tubes and runs linearly between them.

## Notation

Let $f$ be the fluence on the surface. An alive tube $T$ has the corners $x_0, x_1, x_2$, the edges $e_{ij} = x_i - x_j$ and the size $h = \max_{i<j} |e_{ij}|$. A point $x$ of $T$ has the barycentric coordinates $\lambda_i \ge 0$ with $\sum_i \lambda_i = 1$ and $\sum_i \lambda_i x_i = x$. In $T$ the map is the linear interpolation $L(x) = \sum_i \lambda_i F_i$ of the values $F_i$ at the corners. $\|D^2 f\|$ and $\|D^3 f\|$ are the largest second and third derivatives of $f$, as the norms of the symmetric bilinear and trilinear forms, over the region in question.

## Assumptions

- **A1** On the alive tubes of a sheet, $f$ is three times continuously differentiable.
- **A2** The values at the corners are values of the fluence: $F_i = f(x_i)$. Without optics between the source and the surface this holds exactly, since an analysis gives each ray the value of its source's distribution times the area it stands for (see [The value at a corner](../../concepts/fluence.md#the-value-at-a-corner)). Behind optics it holds only approximately (see the limits below).
- **A3** The light does not fold over itself on the surface (see [Folds and caustics](../../concepts/fluence.md#folds-and-caustics)).
- **A4** The light forms one sheet. Overlapping sheets add up point by point, and their estimates with them.

## Theorem 1: error of the linear interpolation

Under A1 and A2, for every point $x$ of $T$

$$f(x) - L(x) = -\frac{1}{2}\sum_{i<j}\lambda_i\lambda_j\, e_{ij}^\top D^2 f(x)\, e_{ij} + R(x), \qquad |R(x)| \le \frac{1}{6}\,\|D^3 f\|\,h^3 .$$

*Proof.* Let $y_i = x_i - x$. Taylor's theorem along the segment from $x$ to $x_i$, which lies in $T$, gives $f(x_i) = f(x) + \nabla f(x)\cdot y_i + \frac{1}{2}\,y_i^\top D^2 f(x)\, y_i + r_i$ with $|r_i| \le \frac{1}{6}\|D^3 f\|\,|y_i|^3 \le \frac{1}{6}\|D^3 f\|\,h^3$, since no point of a triangle lies farther than its longest edge from a corner. Weighted by $\lambda_i$ and summed, the first term gives $f(x)$, and the second vanishes because $\sum_i \lambda_i y_i = 0$. For the third, take any symmetric matrix $A$. Since $\sum_i \lambda_i = 1$ and $\sum_i \lambda_i y_i = 0$,

$$\sum_{i,j}\lambda_i\lambda_j\,(y_i - y_j)^\top A\,(y_i - y_j) = 2\sum_i \lambda_i\, y_i^\top A\, y_i - 2\Big(\sum_i \lambda_i y_i\Big)^\top A\,\Big(\sum_j \lambda_j y_j\Big) = 2\sum_i \lambda_i\, y_i^\top A\, y_i ,$$

and since $y_i - y_j = e_{ij}$, the left side is $2\sum_{i<j}\lambda_i\lambda_j\, e_{ij}^\top A\, e_{ij}$. So $L(x) = \sum_i \lambda_i f(x_i) = f(x) + \frac{1}{2}\sum_{i<j}\lambda_i\lambda_j\, e_{ij}^\top D^2 f(x)\, e_{ij} + \sum_i \lambda_i r_i$, which is the statement with $R = -\sum_i \lambda_i r_i$. $\square$

For a quadratic fluence $D^3 f = 0$, so $R = 0$ and the formula is exact.

## Theorem 2: curvature at a corner

Let a corner $x_0$ share alive tubes with the neighbors $x_1, \dots, x_m$. Let $d_k = x_k - x_0$, $\Delta_k = F_k - F_0$, $s = \max_k |d_k|$ and $\delta_k = d_k / s$. OPOSSUM fits $\Delta_k \approx g\cdot d_k + \frac{1}{2}\, d_k^\top \hat H\, d_k$ by least squares in five unknowns, the gradient $g$ and the symmetric matrix $\hat H$. The fit has the $m \times 5$ matrix $M$ with the rows $\big(\delta_{k,x},\ \delta_{k,y},\ \frac{1}{2}\delta_{k,x}^2,\ \delta_{k,x}\delta_{k,y},\ \frac{1}{2}\delta_{k,y}^2\big)$, and the unknowns $(s\,g,\ s^2 \hat H_{xx},\ s^2 \hat H_{xy},\ s^2 \hat H_{yy})$. Its solution is unique if $M$ has rank 5, which needs $m \ge 5$. OPOSSUM gives a corner no curvature unless the smallest singular value $\sigma_{\min}$ of $M$ exceeds $10^{-10}$ times the largest. Under A2:

1. If $f$ is quadratic, its gradient and Hessian at $x_0$ fit the $\Delta_k$ without residual, so the unique least-squares solution is exact: $\hat H = D^2 f$.
2. Under A1, the $\Delta_k$ equal $\nabla f(x_0)\cdot d_k + \frac{1}{2}\, d_k^\top D^2 f(x_0)\, d_k + \rho_k$ with $|\rho_k| \le \frac{1}{6}\|D^3 f\|\, s^3$, by Taylor's theorem along the edge from $x_0$ to $x_k$. The least-squares solution is linear in the data, so the error of the fit is the least-squares solution for the $\rho_k$ alone, of length at most $|\rho| / \sigma_{\min} \le \sqrt{m}\,\|D^3 f\|\, s^3 / (6\,\sigma_{\min})$. Dividing by $s^2$, every entry of $\hat H - D^2 f(x_0)$ is at most

$$\frac{\sqrt{m}}{6\,\sigma_{\min}}\,\|D^3 f\|\, s .$$

The factor $\sqrt{m}/(6\,\sigma_{\min})$ depends on the shape of the neighborhood, not on its size.

## The estimate

In each alive tube $T$, OPOSSUM takes $\hat H_T$, the mean of $\hat H$ over those of its corners that have one, and draws

$$\hat e(x) = -\frac{1}{2}\sum_{i<j}\lambda_i\lambda_j\, e_{ij}^\top \hat H_T\, e_{ij}$$

onto the points of the map, where overlapping tubes add up as in the map. By Theorem 1, and since $\sum_{i<j}\lambda_i\lambda_j \le \frac{1}{3}$ and $|e_{ij}| \le h$,

$$\big|f(x) - L(x) - \hat e(x)\big| \le \frac{h^2}{6}\,\big\|D^2 f(x) - \hat H_T\big\| + \frac{1}{6}\,\|D^3 f\|\,h^3 .$$

At each corner $x_c$ of $T$, $\|D^2 f(x) - D^2 f(x_c)\| \le \|D^3 f\|\,h$, and by Theorem 2 $\|D^2 f(x_c) - \hat H_c\| \le 2\,C\,\|D^3 f\|\,s$, where $C$ is the largest $\sqrt{m}/(6\,\sigma_{\min})$ and $s$ the largest step of the corners of $T$, and the factor 2 turns the bound on the entries of a symmetric $2 \times 2$ matrix into one on its norm. So

$$\big|f(x) - L(x) - \hat e(x)\big| \le \frac{1}{6}\,\|D^3 f\|\,\big(2h^3 + 2\,C\,h^2 s\big) ,$$

of the order $h^3$. The estimate catches the leading part of the deviation, of the order $h^2$; what it misses is one order smaller. For a quadratic fluence it is exact.

## The peak

Let $P$ be the peak of the map, its highest value over its points $y_k$, spaced $h_r$ apart along both axes. The report gives

$$U = \max\Big(0,\ \max_k \big(L(y_k) + \hat e(y_k)\big) - P\Big).$$

- *The peak of the map does not lie above the peak of the fluence.* Under A2 to A4, every point of the map lies in at most one alive tube or margin, unless a margin reaches across a gap narrower than a tube onto another margin or tube. In a tube the map is an average of corner values, so $L \le \max_i F_i = \max_i f(x_i)$, and a margin carries the values of its edge. Hence $P \le \max f$.
- *The peak of the fluence lies above that of the map by about $U$.* Let the peak of $f$ lie at $x^*$, inside alive tubes that have an estimate and at least $h_r/\sqrt{2}$ from their outer edge, and let $y^*$ be the point of the map nearest to it, so that $|x^* - y^*| \le h_r/\sqrt{2}$ and $y^*$ lies in those tubes too. Since $\nabla f(x^*) = 0$, Taylor's theorem gives $f(x^*) - f(y^*) \le \frac{1}{2}\|D^2 f\|\,|x^* - y^*|^2 \le \frac{1}{4}\|D^2 f\|\,h_r^2$. With the bound $\varepsilon$ of the estimate above, $f(y^*) \le L(y^*) + \hat e(y^*) + \varepsilon \le P + U + \varepsilon$. Together:

$$P \;\le\; \max f \;\le\; P + U + \varepsilon + \tfrac{1}{4}\,\|D^2 f\|\,h_r^2 .$$

$U$ is therefore an estimate, not a bound: the peak of the fluence can lie above $P + U$ by a remainder of the third order in the spacing of the rays and of the second order in the spacing of the map. For a Gaussian beam with σ = 2 mm, sampled by 7 hexapolar rings out to 8 mm, rays about 1.1 mm apart, with its peak 1 mm off the axis, the peak of the map lies 0.187 J/cm² below that of the beam. $U$ = 0.148 J/cm² closes 79 % of the gap; the remaining 0.039 J/cm² is the remainder. Halving the spacing of the rays shrinks the largest deviation of the map within 2σ about fourfold, and what the estimate leaves about eightfold: 0.188, 0.062 and 0.018 J/cm², against 0.040, 0.0044 and 0.0006 J/cm², for 7, 14 and 28 rings.

## Limits

- Behind optics, A2 holds only approximately. A corner's value divides by the growth $D_{det}/D_{src}$ of its alive tubes, an average over the tubes around it rather than the magnification at the ray itself. Its error is of the first order in $h$, and of the second where the tubes lie point-symmetrically around the corner. $U$ does not contain it.
- Margins have no estimate: A1 fails there, because the light ends. Where it ends is known to half a tube, which `Energy in edge margins` tells.
- A corner with fewer than five neighbors along alive tubes, or with a fit of rank below five, has no curvature, as on the outermost ring of a beam. Its tubes use the mean over their other corners, which is taken from further inside and can overshoot where the curvature changes fast; the cut in [Too few rays](../../concepts/fluence.md#too-few-rays) shows this at 7.5 mm. A tube none of whose corners has a curvature has no estimate.
- Where the light folds over itself (A3), where sheets overlap (A4), or where a margin reaches across a gap narrower than a tube, $P \le \max f$ need not hold.
- Nothing finer than the rays is seen. The bounds above are small only if $h\,\|D^3 f\|$ is small against $\|D^2 f\|$, that is if the light changes little from one ray to the next. A structure narrower than the spacing of the rays shows in no estimate built from them.
- The estimate is made for Voronoi maps only; KDE and binning report none.
