use super::*;
use crate::Node;
use bezier_rs::BezierHandles;
use dyn_any::{DynAny, StaticType};
use std::collections::{HashMap, HashSet};
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PointModification {
	add: Vec<PointId>,
	remove: HashSet<PointId>,
	delta: HashMap<PointId, DVec2>,
}

impl PointModification {
	pub fn apply(&self, point_domain: &mut PointDomain) {
		point_domain.retain(|id| !self.remove.contains(id));

		for (id, pos) in point_domain.positions_mut() {
			let Some(&delta) = self.delta.get(&id) else { continue };
			if !delta.is_finite() {
				warn!("invalid delta");
				continue;
			}
			*pos += delta;
		}

		for &add_id in &self.add {
			let Some(&position) = self.delta.get(&add_id) else { continue };
			if !position.is_finite() {
				warn!("invalid position");
				continue;
			}
			point_domain.push(add_id, position);
		}
	}
	pub fn create_from_vector(vector_data: &VectorData) -> Self {
		Self {
			add: vector_data.point_domain.ids().to_vec(),
			remove: HashSet::new(),
			delta: vector_data.point_domain.ids().iter().copied().zip(vector_data.point_domain.positions().iter().cloned()).collect(),
		}
	}
	fn push(&mut self, id: PointId, pos: DVec2) {
		self.add.push(id);
		self.delta.insert(id, pos);
	}
	fn remove(&mut self, id: PointId) {
		self.remove.insert(id);
		self.add.retain(|&add| add != id);
		self.delta.remove(&id);
	}
}
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SegmentModification {
	add: Vec<SegmentId>,
	remove: HashSet<SegmentId>,
	start_point: HashMap<SegmentId, PointId>,
	end_point: HashMap<SegmentId, PointId>,
	handle_primary: HashMap<SegmentId, Option<DVec2>>,
	handle_end: HashMap<SegmentId, Option<DVec2>>,
	stroke: HashMap<SegmentId, StrokeId>,
}

impl SegmentModification {
	pub fn apply(&self, segment_domain: &mut SegmentDomain, point_domain: &PointDomain) {
		segment_domain.retain(|id| !self.remove.contains(id));

		for (id, point) in segment_domain.start_point_mut() {
			let Some(&new) = self.start_point.get(&id) else { continue };
			if !point_domain.ids().contains(&new) {
				warn!("invalid start id");
				continue;
			}
			*point = new;
		}
		for (id, point) in segment_domain.end_point_mut() {
			let Some(&new) = self.end_point.get(&id) else { continue };
			if !point_domain.ids().contains(&new) {
				warn!("invalid end id");
				continue;
			}
			*point = new;
		}
		for (id, handles, start, end) in segment_domain.handles_mut() {
			let Some(start) = point_domain.pos_from_id(start) else { continue };
			let Some(end) = point_domain.pos_from_id(end) else { continue };
			let start = self.handle_primary.get(&id).copied().map(|handle| handle.map(|handle| handle + start));
			let end = self.handle_end.get(&id).copied().map(|handle| handle.map(|handle| handle + end));

			if !start.unwrap_or_default().map_or(true, |start| start.is_finite()) || !end.unwrap_or_default().map_or(true, |end| end.is_finite()) {
				warn!("invalid handles");
				continue;
			}
			match (start, end) {
				(Some(Some(handle_start)), Some(Some(handle_end))) => *handles = BezierHandles::Cubic { handle_start, handle_end },
				(Some(Some(handle)), Some(None)) | (Some(None), Some(Some(handle))) => *handles = BezierHandles::Quadratic { handle },
				(Some(None), Some(None)) => *handles = BezierHandles::Linear,

				(None, Some(None)) => {
					if let BezierHandles::Cubic { handle_start, .. } = *handles {
						*handles = BezierHandles::Quadratic { handle: handle_start }
					}
				}
				(None, Some(Some(handle_end))) => match *handles {
					BezierHandles::Linear => *handles = BezierHandles::Quadratic { handle: handle_end },
					BezierHandles::Quadratic { handle: handle_start } => *handles = BezierHandles::Cubic { handle_start, handle_end },
					BezierHandles::Cubic { handle_start, .. } => *handles = BezierHandles::Cubic { handle_start, handle_end },
				},

				(Some(None), None) => *handles = BezierHandles::Linear,
				(Some(Some(handle_start)), None) => match *handles {
					BezierHandles::Linear => *handles = BezierHandles::Quadratic { handle: handle_start },
					BezierHandles::Quadratic { .. } => *handles = BezierHandles::Quadratic { handle: handle_start },
					BezierHandles::Cubic { handle_end, .. } => *handles = BezierHandles::Cubic { handle_start, handle_end },
				},

				(None, None) => {}
			};
		}
		for (id, stroke) in segment_domain.stroke_mut() {
			let Some(&new) = self.stroke.get(&id) else { continue };
			*stroke = new;
		}

		for &add_id in &self.add {
			let Some(&start) = self.start_point.get(&add_id) else { continue };
			let Some(&end) = self.end_point.get(&add_id) else { continue };
			let Some(&handle_start) = self.handle_primary.get(&add_id) else { continue };
			let Some(&handle_end) = self.handle_end.get(&add_id) else { continue };
			let Some(&stroke) = self.stroke.get(&add_id) else { continue };
			if !point_domain.ids().contains(&start) {
				warn!("invalid start id");
				continue;
			}
			if !point_domain.ids().contains(&end) {
				warn!("invalid end id");
				continue;
			}
			let Some(start_position) = point_domain.pos_from_id(start) else { continue };
			let Some(end_position) = point_domain.pos_from_id(end) else { continue };
			let handles = match (handle_start.map(|handle| handle + start_position), handle_end.map(|handle| handle + end_position)) {
				(Some(handle_start), Some(handle_end)) => BezierHandles::Cubic { handle_start, handle_end },
				(Some(handle), None) | (None, Some(handle)) => BezierHandles::Quadratic { handle },
				(None, None) => BezierHandles::Linear,
			};
			if !handles.is_finite() {
				warn!("invalid handles");
				continue;
			}
			segment_domain.push(add_id, start, end, handles, stroke);
		}
	}
	pub fn create_from_vector(vector_data: &VectorData) -> Self {
		Self {
			add: vector_data.segment_domain.ids().to_vec(),
			remove: HashSet::new(),
			start_point: vector_data.segment_domain.ids().iter().copied().zip(vector_data.segment_domain.start_point().iter().cloned()).collect(),
			end_point: vector_data.segment_domain.ids().iter().copied().zip(vector_data.segment_domain.end_point().iter().cloned()).collect(),
			handle_primary: vector_data.segment_bezier_iter().map(|(id, b, _, _)| (id, b.handle_start().map(|handle| handle - b.start))).collect(),
			handle_end: vector_data.segment_bezier_iter().map(|(id, b, _, _)| (id, b.handle_end().map(|handle| handle - b.end))).collect(),
			stroke: vector_data.segment_domain.ids().iter().copied().zip(vector_data.segment_domain.stroke().iter().cloned()).collect(),
		}
	}
	fn push(&mut self, id: SegmentId, points: [PointId; 2], handles: [Option<DVec2>; 2], stroke: StrokeId) {
		self.add.push(id);
		self.start_point.insert(id, points[0]);
		self.end_point.insert(id, points[1]);
		self.handle_primary.insert(id, handles[0]);
		self.handle_end.insert(id, handles[1]);
		self.stroke.insert(id, stroke);
	}
	fn remove(&mut self, id: SegmentId) {
		self.remove.insert(id);
		self.add.retain(|&add| add != id);
		self.start_point.remove(&id);
		self.end_point.remove(&id);
		self.handle_primary.remove(&id);
		self.handle_end.remove(&id);
		self.stroke.remove(&id);
	}
}
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RegionModification {
	add: Vec<RegionId>,
	remove: HashSet<RegionId>,
	segment_range: HashMap<RegionId, core::ops::RangeInclusive<SegmentId>>,
	fill: HashMap<RegionId, FillId>,
}

impl RegionModification {
	pub fn apply(&self, region_domain: &mut RegionDomain) {
		region_domain.retain(|id| !self.remove.contains(id));

		for (id, segment_range) in region_domain.segment_range_mut() {
			let Some(new) = self.segment_range.get(&id) else { continue };
			*segment_range = new.clone(); // Range inclusive is not copy
		}
		for (id, fill) in region_domain.fill_mut() {
			let Some(&new) = self.fill.get(&id) else { continue };
			*fill = new;
		}

		for &add_id in &self.add {
			let Some(segment_range) = self.segment_range.get(&add_id) else { continue };
			let Some(&fill) = self.fill.get(&add_id) else { continue };
			region_domain.push(add_id, segment_range.clone(), fill);
		}
	}
	pub fn create_from_vector(vector_data: &VectorData) -> Self {
		Self {
			add: vector_data.region_domain.ids().to_vec(),
			remove: HashSet::new(),
			segment_range: vector_data.region_domain.ids().iter().copied().zip(vector_data.region_domain.segment_range().iter().cloned()).collect(),
			fill: vector_data.region_domain.ids().iter().copied().zip(vector_data.region_domain.fill().iter().cloned()).collect(),
		}
	}
}
#[derive(Clone, Debug, Default, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VectorModification {
	points: PointModification,
	segments: SegmentModification,
	regions: RegionModification,
	add_g1_continous: HashSet<[HandleId; 2]>,
	remove_g1_continous: HashSet<[HandleId; 2]>,
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum VectorModificationType {
	InsertSegment { id: SegmentId, points: [PointId; 2], handles: [Option<DVec2>; 2] },
	InsertPoint { id: PointId, pos: DVec2 },

	RemoveSegment { id: SegmentId },
	RemovePoint { id: PointId },

	SetG1Continous { handles: [HandleId; 2], enabled: bool },
	SetHandles { segment: SegmentId, handles: [Option<DVec2>; 2] },
	SetPrimaryHandle { segment: SegmentId, pos: DVec2 },
	SetEndHandle { segment: SegmentId, pos: DVec2 },
	SetStartPoint { segment: SegmentId, id: PointId },
	SetEndPoint { segment: SegmentId, id: PointId },

	ApplyPointDelta { point: PointId, delta: DVec2 },
	ApplyPrimaryDelta { segment: SegmentId, delta: DVec2 },
	ApplyEndDelta { segment: SegmentId, delta: DVec2 },
}

impl VectorModification {
	pub fn apply(&self, vector_data: &mut VectorData) {
		self.points.apply(&mut vector_data.point_domain);
		self.segments.apply(&mut vector_data.segment_domain, &vector_data.point_domain);
		self.regions.apply(&mut vector_data.region_domain);
		vector_data
			.colinear_manipulators
			.retain(|val| !self.remove_g1_continous.contains(val) && !self.remove_g1_continous.contains(&[val[1], val[0]]));
		for handles in &self.add_g1_continous {
			if !vector_data.colinear_manipulators.iter().any(|test| test == handles || test == &[handles[1], handles[0]]) {
				vector_data.colinear_manipulators.push(*handles);
			}
		}
	}
	pub fn modify(&mut self, vector_data_modification: &VectorModificationType) {
		match vector_data_modification {
			VectorModificationType::InsertSegment { id, points, handles } => self.segments.push(*id, *points, *handles, StrokeId::ZERO),
			VectorModificationType::InsertPoint { id, pos } => self.points.push(*id, *pos),

			VectorModificationType::RemoveSegment { id } => self.segments.remove(*id),
			VectorModificationType::RemovePoint { id } => self.points.remove(*id),

			VectorModificationType::SetG1Continous { handles, enabled } => {
				if *enabled {
					if !self.add_g1_continous.contains(&[handles[1], handles[0]]) {
						self.add_g1_continous.insert(*handles);
					}
					self.remove_g1_continous.remove(handles);
					self.remove_g1_continous.remove(&[handles[1], handles[0]]);
				} else {
					if !self.remove_g1_continous.contains(&[handles[1], handles[0]]) {
						self.remove_g1_continous.insert(*handles);
					}
					self.add_g1_continous.remove(handles);
					self.add_g1_continous.remove(&[handles[1], handles[0]]);
				}
			}
			VectorModificationType::SetHandles { segment, handles } => {
				self.segments.handle_primary.insert(*segment, handles[0]);
				self.segments.handle_end.insert(*segment, handles[1]);
			}
			VectorModificationType::SetPrimaryHandle { segment, pos } => {
				self.segments.handle_primary.insert(*segment, Some(*pos));
			}
			VectorModificationType::SetEndHandle { segment, pos } => {
				self.segments.handle_end.insert(*segment, Some(*pos));
			}
			VectorModificationType::SetStartPoint { segment, id } => {
				self.segments.start_point.insert(*segment, *id);
			}
			VectorModificationType::SetEndPoint { segment, id } => {
				self.segments.end_point.insert(*segment, *id);
			}

			VectorModificationType::ApplyPointDelta { point, delta } => {
				*self.points.delta.entry(*point).or_default() += *delta;
			}
			VectorModificationType::ApplyPrimaryDelta { segment, delta } => {
				let pos = self.segments.handle_primary.entry(*segment).or_default();
				*pos = Some(pos.unwrap_or_default() + *delta);
			}
			VectorModificationType::ApplyEndDelta { segment, delta } => {
				let pos = self.segments.handle_end.entry(*segment).or_default();
				*pos = Some(pos.unwrap_or_default() + *delta);
			}
		}
	}

	pub fn create_from_vector(vector_data: &VectorData) -> Self {
		Self {
			points: PointModification::create_from_vector(vector_data),
			segments: SegmentModification::create_from_vector(vector_data),
			regions: RegionModification::create_from_vector(vector_data),
			add_g1_continous: vector_data.colinear_manipulators.iter().copied().collect(),
			remove_g1_continous: HashSet::new(),
		}
	}
}

impl core::hash::Hash for VectorModification {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		// TODO: properly implement (hashing a hashset is difficult because ordering is unstable)
		PointId::generate().hash(state);
	}
}

#[derive(Debug, Clone, Copy)]
pub struct PathModify<VectorModificationNode> {
	modification: VectorModificationNode,
}

#[node_macro::node_fn(PathModify)]
fn path_modify(mut vector_data: VectorData, modification: VectorModification) -> VectorData {
	modification.apply(&mut vector_data);
	vector_data
}

#[test]
fn modify_new() {
	let vector_data = VectorData::from_subpaths(
		[bezier_rs::Subpath::new_ellipse(DVec2::ZERO, DVec2::ONE), bezier_rs::Subpath::new_rect(DVec2::NEG_ONE, DVec2::ZERO)],
		false,
	);

	let modify = VectorModification::create_from_vector(&vector_data);

	let mut new = VectorData::empty();
	modify.apply(&mut new);
	assert_eq!(vector_data, new);
}
