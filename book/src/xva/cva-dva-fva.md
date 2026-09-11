# CVA, DVA and FVA

Credit valuation adjustment (CVA) reflects counterparty default loss on positive exposure. Debit valuation adjustment (DVA) reflects own-default benefit on negative exposure. Funding valuation adjustment (FVA) reflects the cost or benefit of funding uncollateralized exposure.

In discrete form, a typical unilateral CVA approximation is

\[
\operatorname{CVA}=(1-R_c)\sum_k
D(0,t_k)\operatorname{EPE}(t_k)\Delta PD_c(t_k),
\]

where \(R_c\) is counterparty recovery. DVA applies an analogous calculation to negative exposure and own default probabilities. FVA integrates exposure against funding spreads.

QuantSupport expresses these calculations through aggregators and factories. `CreditCurveCvaFactory` derives default weighting from a credit curve; `FundingCurveFvaFactory` applies funding information; DVA has its own factory and aggregator interfaces.

Sign conventions differ between reporting systems. Establish whether adjustments are reported as signed contributions or positive costs, then reconcile clean value, each component, and adjusted value. The `cva` example prints the library's concrete convention alongside exposure profiles.
