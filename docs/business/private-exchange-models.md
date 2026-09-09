# Models of private exchange


Settlement risk and information leakage are separate costs. Atomicity removes the asynchrony that creates principal risk; confidential notes conceal positions inside the pool. Neither removes the information carried by aggregate flow, and the FX proposal's market design addresses that residual ([private FX settlement][fx]). The derivations below restate it with the assumptions in view.

**Impact is information.** In Kyle's one-period model the terminal payoff $F\sim\mathcal N(\mu,\Sigma_0)$ is independent of uninformed flow $Z\sim\mathcal N(0,\sigma_u^2)$. An informed trader who observes $F$ submits $X=\beta(F-\mu)$; a competitive market maker sees only $Y=X+Z$ and quotes its conditional expectation,

$$P(Y)=\mathbb E[F\mid Y]=\mu+\lambda Y,\qquad \lambda=\frac{\beta\Sigma_0}{\beta^2\Sigma_0+\sigma_u^2}.$$

Taking the linear rule as given, the trader maximizes $X(F-\mu)-\lambda X^2$, so $X=(F-\mu)/2\lambda$ and $\beta=1/2\lambda$. Substituting into the pricing rule gives $\beta^2\Sigma_0=\sigma_u^2$, hence

$$\beta=\frac{\sigma_u}{\sqrt{\Sigma_0}},\qquad \lambda=\frac{\sqrt{\Sigma_0}}{2\sigma_u}.$$

Size moves price because observable flow reveals information; greater value uncertainty raises impact, and more uninformed flow lowers it. Crossing orders inside a batch shrinks the flow exposed to that inference. This is a mechanism model under Gaussian flow and competitive pricing; it calibrates nothing about Post Fiat's market.

**Netting residual.** Let $n$ signed orders $X_i$ be independent with mean zero, variance $\sigma_X^2$, and $\mathbb E|X_i|=\mu_{\text{abs}}$. Gross flow $G=\sum_i|X_i|$ has mean $n\mu_{\text{abs}}$; the net $N=\sum_iX_i$ is approximately $\mathcal N(0,n\sigma_X^2)$ by the central limit theorem, with mean absolute value $\sqrt{2/\pi}\,\sigma_X\sqrt n$. Hence

$$\frac{\mathbb E|N|}{\mathbb E[G]}\approx\sqrt{\frac{2}{\pi}}\;\frac{\sigma_X}{\mu_{\text{abs}}\sqrt n}.$$

If the magnitudes $|X_i|$ have coefficient of variation one—exponentially distributed sizes, say—then $\mathrm{Var}|X_i|=\mu_{\text{abs}}^2$, so $\sigma_X^2=\mathbb E[X_i^2]=2\mu_{\text{abs}}^2$ and the ratio becomes $2/\sqrt{\pi n}$: about 36% at $n=10$, 11% at $n=100$, and 3.6% at $n=1000$. Persistently one-sided flow with buy probability $p$ leaves a floor of $|2p-1|$ at any $n$; heavy tails, unequal sizes, and a thin batch weaken the result. Netting hides composition and reveals direction.

**What the residual reveals.** With equal-variance Gaussian orders, the published net $N=X_i+R$ is a Gaussian channel with signal $\sigma^2$ and independent noise $(n-1)\sigma^2$:

$$I(X_i;N)=\tfrac12\ln\!\Big(1+\frac{1}{n-1}\Big)=\tfrac12\ln\frac{n}{n-1}\approx\frac{1}{2n}\ \text{nats},$$

about a hundredth of a nat per batch at $n=51$. With unequal variances the term is $\tfrac12\ln(1+\sigma_i^2/\sigma_R^2)$, where $\sigma_R^2$ is the rest of the batch; a participant who dominates batch variance receives little cover, which motivates size limits. The bound concerns one batch residual and one signed order. Quotes, repeated participation, timing, and an operator who sees the book are separate channels.

**Batch length.** Model per-unit cost as staleness plus residual impact, $\text{cost}(\tau)=a\sqrt\tau+b/\sqrt{\Lambda\tau}$, with arrival rate $\Lambda$. Setting the derivative to zero gives

$$\tau^*=\frac{b}{a\sqrt\Lambda},$$

where both terms equal $\sqrt{ab/\sqrt\Lambda}$. Busier venues run shorter batches with more depth ($n^*=\Lambda\tau^*$ grows like $\sqrt\Lambda$), and volatile days want shorter batches. The FX post's seven-minute illustration is a model number. The boundary between demonstrated and proposed is sharp: bilateral atomic settlement under a public, expiring, capacity-bounded fixed quote has run on the controlled devnet with mainnet USDC on the dollar leg and sandbox WNOK on the krone leg, while the shielded frequent batch auction is proposed and its first version requires the matcher to see the book for one interval ([private FX settlement][fx]; [pNOK article][pnok-blog]).

## Sources

Albert S. Kyle, “Continuous Auctions and Insider Trading,” *Econometrica* 53(6), 1985. Eric Budish, Peter Cramton, and John Shim, “The High-Frequency Trading Arms Race: Frequent Batch Auctions as a Market Design Response,” *Quarterly Journal of Economics* 130(4), 2015.

[fx]: https://postfiat.org/private-fx-settlement/
[pnok-blog]: https://postfiat.org/private-fx-executed-pnok/
