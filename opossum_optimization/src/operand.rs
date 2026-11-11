use crate::{address::MetricSource, reader::MetricReader};
pub use opossum_core::opm_document::optimization::operand::*;
use opossum_core::{
    core_optics::NodeAttrExt,
    error::{OpmResult, OpossumError},
    opm_document::OpmDocument,
};
use std::fmt::Debug;

/// Context provided to operands during merit function evaluation.
pub struct EvaluationContext<'a> {
    pub doc: &'a OpmDocument,
}

impl<'a> EvaluationContext<'a> {
    #[must_use]
    pub const fn new(doc: &'a OpmDocument) -> Self {
        Self { doc }
    }

    pub fn read_source(&self, source: &MetricSource) -> OpmResult<f64> {
        match source {
            MetricSource::Observable(addr) => MetricReader::read_metric(self.doc, addr),
            MetricSource::Parameter(addr) => MetricReader::read_parameter_value(self.doc, addr),
        }
    }
}

pub trait Operand: Send + Sync + Debug {
    /// Human-readable descriptor of the operand for logging and diagnostics.
    fn name(&self) -> String;
    fn weight(&self) -> f64;
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64>;

    /// Returns the constituent residual values r_i such that sum(r_i^2) == evaluate().
    /// Defaults to returning a single scalar residual sqrt(cost).
    fn residuals(&self, ctx: &EvaluationContext<'_>) -> OpmResult<Vec<f64>> {
        self.evaluate(ctx).map(|c| vec![c.max(0.0).sqrt()])
    }

    /// Returns a single scalar residual sqrt(cost).
    fn residual(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        self.evaluate(ctx).map(|c| c.max(0.0).sqrt())
    }

    fn validate(&self, _doc: &OpmDocument) -> OpmResult<()> {
        Ok(())
    }
}

impl Operand for MetricOperand {
    fn name(&self) -> String {
        MetricOperand::name(self)
    }
    fn weight(&self) -> f64 {
        MetricOperand::weight(self)
    }
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        let value = ctx.read_source(&self.source)?;
        let violation = match self.mode() {
            OperandMode::Equal => value - self.target,
            OperandMode::UpperLimit => (value - self.target).max(0.0),
            OperandMode::LowerLimit => (self.target - value).max(0.0),
        };
        let normalized = violation / self.scale();
        Ok(self.weight() * normalized * normalized)
    }
    fn residual(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        let value = ctx.read_source(&self.source)?;
        let diff = match self.mode() {
            OperandMode::Equal => value - self.target,
            OperandMode::UpperLimit => (value - self.target).max(0.0),
            OperandMode::LowerLimit => (self.target - value).max(0.0),
        };
        Ok(self.weight().sqrt() * (diff / self.scale()))
    }
    fn residuals(&self, ctx: &EvaluationContext<'_>) -> OpmResult<Vec<f64>> {
        // Try reading multi-ray residuals if available and mode is Equal with target 0.0
        if self.mode == OperandMode::Equal && self.target.abs() < 1e-12 {
            if let MetricSource::Observable(addr) = &self.source {
                if let Ok(vec_data) = MetricReader::read_metric_vector(ctx.doc, addr) {
                    if vec_data.len() > 1 {
                        let n_rays = vec_data.len() as f64;
                        // Each residual scaled by sqrt(weight / N)
                        let factor = (self.weight / n_rays).sqrt() / self.scale();
                        return Ok(vec_data.into_iter().map(|d| d * factor).collect());
                    }
                }
            }
        }

        // Fallback for scalar parameters or unequal targets
        let value = ctx.read_source(&self.source)?;
        let diff = match self.mode {
            OperandMode::Equal => value - self.target,
            OperandMode::UpperLimit => (value - self.target).max(0.0),
            OperandMode::LowerLimit => (self.target - value).max(0.0),
        };
        Ok(vec![self.weight.sqrt() * (diff / self.scale())])
    }
}

impl Operand for MaxTotalLengthOperand {
    fn name(&self) -> String {
        MaxTotalLengthOperand::name(self).into_owned()
    }
    fn weight(&self) -> f64 {
        MaxTotalLengthOperand::weight(self)
    }
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        let mut total = 0.0;
        for seg in &self.segments {
            total += MetricReader::read_parameter_value(ctx.doc, seg)?;
        }
        let violation = (total - self.max_length).max(0.0);
        let normalized = violation / self.scale();
        Ok(self.weight * normalized * normalized)
    }
}

impl Operand for MinEdgeThicknessOperand {
    fn name(&self) -> String {
        MinEdgeThicknessOperand::name(self)
    }
    fn weight(&self) -> f64 {
        MinEdgeThicknessOperand::weight(self)
    }
    fn validate(&self, doc: &OpmDocument) -> OpmResult<()> {
        let (node_ref, _) = doc.scenery().node_recursive(self.node_id)?;
        if node_ref.as_edge_thickness().is_none() {
            return Err(OpossumError::Other(format!(
                "Node '{}' ({}) does not support edge thickness evaluation",
                node_ref.name(),
                node_ref.node_type()
            )));
        }
        Ok(())
    }
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        let (node_ref, _) = ctx.doc.scenery().node_recursive(self.node_id)?;
        let edge_thickness = node_ref
            .as_edge_thickness()
            .ok_or_else(|| {
                OpossumError::Other(format!(
                    "Node '{}' does not implement EdgeThickness",
                    node_ref.name()
                ))
            })?
            .min_edge_thickness()?
            .value;
        let violation = (self.min_thickness - edge_thickness).max(0.0);
        let normalized = violation / self.scale();
        Ok(self.weight * normalized * normalized)
    }
}

impl Operand for TransmissionPenaltyOperand {
    fn name(&self) -> String {
        TransmissionPenaltyOperand::name(self)
    }
    fn weight(&self) -> f64 {
        TransmissionPenaltyOperand::weight(self)
    }
    fn validate(&self, doc: &OpmDocument) -> OpmResult<()> {
        let _ = doc.scenery().node_recursive(self.address.node_id)?;
        if !doc.analyzers().contains_key(&self.address.analyzer_id) {
            return Err(OpossumError::Other(format!(
                "Analyzer '{}' not found in document",
                self.address.analyzer_id
            )));
        }
        Ok(())
    }
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        if self.expected_rays <= 0.0 {
            return Ok(0.0);
        }
        let valid_rays = MetricReader::read_metric(ctx.doc, &self.address)?;
        if valid_rays >= self.expected_rays {
            return Ok(0.0);
        }
        let loss_ratio = (self.expected_rays - valid_rays) / self.expected_rays;
        Ok(self.weight * loss_ratio * loss_ratio)
    }
}

impl Operand for StandardOperand {
    fn name(&self) -> String {
        StandardOperand::name(self)
    }
    fn weight(&self) -> f64 {
        StandardOperand::weight(self)
    }
    fn validate(&self, doc: &OpmDocument) -> OpmResult<()> {
        match self {
            Self::Metric(op) => op.validate(doc),
            Self::MaxTotalLength(op) => op.validate(doc),
            Self::MinEdgeThickness(op) => op.validate(doc),
            Self::TransmissionPenalty(op) => op.validate(doc),
        }
    }
    fn evaluate(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        match self {
            Self::Metric(op) => op.evaluate(ctx),
            Self::MaxTotalLength(op) => op.evaluate(ctx),
            Self::MinEdgeThickness(op) => op.evaluate(ctx),
            Self::TransmissionPenalty(op) => op.evaluate(ctx),
        }
    }
    fn residual(&self, ctx: &EvaluationContext<'_>) -> OpmResult<f64> {
        match self {
            Self::Metric(op) => op.residual(ctx),
            Self::MaxTotalLength(op) => op.residual(ctx),
            Self::MinEdgeThickness(op) => op.residual(ctx),
            Self::TransmissionPenalty(op) => op.residual(ctx),
        }
    }
    fn residuals(&self, ctx: &EvaluationContext<'_>) -> OpmResult<Vec<f64>> {
        match self {
            Self::Metric(op) => op.residuals(ctx),
            Self::MaxTotalLength(op) => op.residuals(ctx),
            Self::MinEdgeThickness(op) => op.residuals(ctx),
            Self::TransmissionPenalty(op) => op.residuals(ctx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use opossum_core::{
        analyzers::{AnalyzerType, RayTraceConfig},
        distributions::{energy::UniformDist, position::Hexapolar, spectral::LaserLines},
        joule, millimeter, nanometer,
        nodes::{NodeGroup, SourcePort, WaveFront},
        opm_document::OpmDocument,
        prelude::{PointSrc, RayDataSource},
    };
    use crate::{address::ObservableAddress, variable::FreeVariable};
    use std::sync::Arc;

    #[test]
    fn test_wavefront_vector_residuals_match_evaluation() -> OpmResult<()> {
        let mut scenery = NodeGroup::new("WaveFront test setup");
        let i_src = scenery.add_node(SourcePort::new("source"))?;
        let i_wf = scenery.add_node(WaveFront::new("wf")?)?;
        scenery.connect_nodes(i_src, "output_1", i_wf, "input_1", millimeter!(50.0))?;

        let mut doc = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(
            i_src,
            RayDataSource::PointSrc(PointSrc::new(
                Hexapolar::new(millimeter!(5.0), 2)?.into(),
                UniformDist::new(joule!(1.0))?.into(),
                LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
                millimeter!(50.0),
            )?)
            .into(),
        );
        let analyzer_id = doc.add_analyzer(AnalyzerType::RayTrace(config));
        doc.analyze_headless()?;

        let addr = ObservableAddress {
            analyzer_id,
            node_id: i_wf,
            metric_name: "wavefront rms".into(),
        };

        let operand = MetricOperand::new(addr.clone(), 0.0, OperandMode::Equal, 2.5, 0.01);
        let ctx = EvaluationContext::new(&doc);

        let scalar_cost = operand.evaluate(&ctx)?;
        let residuals = operand.residuals(&ctx)?;

        // Hexapolar with 2 rings has 19 rays
        assert_eq!(residuals.len(), 19);
        let sum_sq_residuals: f64 = residuals.iter().map(|&r| r * r).sum();
        assert_relative_eq!(scalar_cost, sum_sq_residuals, max_relative = 1e-12);

        // Fallback when target != 0.0
        let operand_nonzero = MetricOperand::new(addr, 0.1, OperandMode::Equal, 2.5, 0.01);
        let residuals_nonzero = operand_nonzero.residuals(&ctx)?;
        assert_eq!(residuals_nonzero.len(), 1);
        let single_sq = residuals_nonzero[0] * residuals_nonzero[0];
        assert_relative_eq!(operand_nonzero.evaluate(&ctx)?, single_sq, max_relative = 1e-12);

        Ok(())
    }

    #[test]
    fn test_merit_function_with_wavefront_residuals_and_jacobian() -> OpmResult<()> {
        let mut scenery = NodeGroup::new("MeritFunction WaveFront setup");
        let i_src = scenery.add_node(SourcePort::new("source"))?;
        let i_wf = scenery.add_node(WaveFront::new("wf")?)?;
        scenery.connect_nodes(i_src, "output_1", i_wf, "input_1", millimeter!(50.0))?;

        let mut doc = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(
            i_src,
            RayDataSource::PointSrc(PointSrc::new(
                Hexapolar::new(millimeter!(5.0), 2)?.into(),
                UniformDist::new(joule!(1.0))?.into(),
                LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
                millimeter!(50.0),
            )?)
            .into(),
        );
        let analyzer_id = doc.add_analyzer(AnalyzerType::RayTrace(config));

        let addr = ObservableAddress {
            analyzer_id,
            node_id: i_wf,
            metric_name: "wavefront rms".into(),
        };

        let operand = Arc::new(MetricOperand::new(addr, 0.0, OperandMode::Equal, 1.0, 1.0));
        let var = FreeVariable::new(crate::address::ParameterAddress::ConnectionDistance {
            src_id: i_src,
            src_port: "output_1".into(),
        });

        let merit = crate::merit::MeritFunction::new(doc, vec![var], vec![], vec![operand])?;
        let x = [0.050];
        let residuals = merit.evaluate_residuals(&x);

        // 19 ray residuals + 1 parameter bound residual = 20 residuals
        assert_eq!(residuals.len(), 20);

        let jacobian = merit.compute_jacobian(&x, &residuals);
        assert_eq!(jacobian.nrows(), 20);
        assert_eq!(jacobian.ncols(), 1);

        Ok(())
    }
}
