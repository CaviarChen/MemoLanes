use std::collections::HashSet;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;

use memolanes_core::api::api::{self, ExportType};
use memolanes_core::api::edit_session::{AddLinesOutcome, EditSession};
use memolanes_core::api::import::{self, ImportPreprocessor};
use memolanes_core::export_data::gpx::journey_vector_to_gpx_file;
use memolanes_core::journey_vector::{JourneyVector, TrackPoint, TrackSegment};
use tempdir::TempDir;

fn point(latitude: f64, longitude: f64) -> TrackPoint {
    TrackPoint {
        latitude,
        longitude,
    }
}

fn vector(segments: &[&[(f64, f64)]]) -> JourneyVector {
    JourneyVector {
        track_segments: segments
            .iter()
            .map(|segment| TrackSegment {
                track_points: segment
                    .iter()
                    .map(|&(latitude, longitude)| point(latitude, longitude))
                    .collect(),
            })
            .collect(),
    }
}

fn initialize_app() -> TempDir {
    let root = TempDir::new("edit-session").unwrap();
    let directory = |name: &str| {
        let path = root.path().join(name);
        fs::create_dir(&path).unwrap();
        path.into_os_string().into_string().unwrap()
    };

    api::init(
        directory("temp"),
        directory("doc"),
        directory("support"),
        directory("cache"),
    );
    root
}

fn import_vector(root: &Path, name: &str, journey: &JourneyVector) -> String {
    let existing_ids: HashSet<_> = api::list_all_journeys()
        .unwrap()
        .into_iter()
        .map(|header| header.id)
        .collect();
    let path = root.join(format!("{name}.gpx"));
    journey_vector_to_gpx_file(journey, &mut File::create(&path).unwrap()).unwrap();

    let (info, raw_data, _) = import::load_gpx_or_kml(path.to_string_lossy().into_owned()).unwrap();
    let journey_data = import::process_vector_data(&raw_data, ImportPreprocessor::None).unwrap();
    import::import_journey_data(info, journey_data).unwrap();

    api::list_all_journeys()
        .unwrap()
        .into_iter()
        .find(|header| !existing_ids.contains(&header.id))
        .unwrap()
        .id
}

fn export_vector(root: &Path, name: &str, journey_id: &str) -> JourneyVector {
    let path = root.join(format!("{name}.gpx"));
    api::export_journey(
        path.to_string_lossy().into_owned(),
        journey_id.to_owned(),
        ExportType::GPX,
    )
    .unwrap();

    let mut reader = BufReader::new(File::open(path).unwrap());
    let gpx = gpx::read(&mut reader).unwrap();
    JourneyVector {
        track_segments: gpx.tracks[0]
            .segments
            .iter()
            .map(|segment| TrackSegment {
                track_points: segment
                    .points
                    .iter()
                    .map(|waypoint| {
                        let point = waypoint.point();
                        TrackPoint {
                            latitude: point.y(),
                            longitude: point.x(),
                        }
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn verify_linked_drawing(root: &Path) {
    let one_track = vector(&[&[(0.0, 0.0), (0.0, 1.0)]]);
    let one_track_id = import_vector(root, "one-track", &one_track);
    let mut one_track_session = EditSession::new(one_track_id).unwrap().unwrap();
    assert_eq!(
        one_track_session
            .add_lines(&[(0.0, 0.0), (1.0, 1.0)], true)
            .unwrap(),
        AddLinesOutcome::LinkedDrawNeedsMultipleTracks
    );
    assert!(!one_track_session.can_undo());

    let original = vector(&[&[(0.0, 0.0), (0.0, 1.0)], &[(1.0, 0.0), (1.0, 1.0)]]);
    let journey_id = import_vector(root, "drawing", &original);
    let mut session = EditSession::new(journey_id.clone()).unwrap().unwrap();

    assert_eq!(
        session
            .add_lines(&[(0.5, 0.5), (0.5001, 0.5001)], true)
            .unwrap(),
        AddLinesOutcome::LinkedDrawTooFar
    );
    assert_eq!(
        session
            .add_lines(&[(0.0, 0.01), (0.0, 0.99)], true)
            .unwrap(),
        AddLinesOutcome::LinkedDrawInvalidLinkTargets
    );
    assert_eq!(
        session
            .add_lines(&[(2.0, 2.0), (2.0, 2.0 + 5e-8)], false)
            .unwrap(),
        AddLinesOutcome::Ignored
    );
    assert!(!session.can_undo());

    assert_eq!(
        session
            .add_lines(&[(2.0, 2.0), (2.0, 2.0 + 2e-7)], false)
            .unwrap(),
        AddLinesOutcome::Added
    );
    session.undo().unwrap();
    assert!(!session.can_undo());

    let linked_line = [(0.01, 0.99), (0.99, 0.99)];
    assert_eq!(
        session.add_lines(&linked_line, true).unwrap(),
        AddLinesOutcome::Added
    );
    assert!(session.can_undo());
    session.undo().unwrap();
    assert!(!session.can_undo());

    assert_eq!(
        session.add_lines(&linked_line, true).unwrap(),
        AddLinesOutcome::Added
    );
    session.commit().unwrap();

    let mut expected = original.clone();
    expected.track_segments.push(TrackSegment {
        track_points: vec![point(0.0, 1.0), point(1.0, 1.0)],
    });
    assert_eq!(export_vector(root, "drawing-result", &journey_id), expected);
}

fn verify_box_deletion(root: &Path) {
    let crossing = vector(&[&[(0.0, -2.0), (0.0, 2.0)]]);
    let crossing_id = import_vector(root, "crossing", &crossing);
    let mut session = EditSession::new(crossing_id.clone()).unwrap().unwrap();
    session.delete_points_in_box(-1.0, -1.0, 1.0, 1.0).unwrap();
    assert!(session.can_undo());
    session.undo().unwrap();
    assert!(!session.can_undo());

    session.delete_points_in_box(-1.0, -1.0, 1.0, 1.0).unwrap();
    session.commit().unwrap();
    assert_eq!(
        export_vector(root, "crossing-result", &crossing_id),
        vector(&[&[(0.0, -2.0), (0.0, -1.0)], &[(0.0, 1.0), (0.0, 2.0)],])
    );

    let mut no_op_session = EditSession::new(crossing_id).unwrap().unwrap();
    no_op_session
        .delete_points_in_box(20.0, 20.0, 21.0, 21.0)
        .unwrap();
    assert!(!no_op_session.can_undo());
}

fn verify_antimeridian_snapping(root: &Path) {
    let antimeridian = vector(&[
        &[(10.0, 170.0), (10.0, 179.9)],
        &[(11.0, -170.0), (11.0, -179.9)],
    ]);
    let antimeridian_id = import_vector(root, "antimeridian", &antimeridian);
    let mut session = EditSession::new(antimeridian_id.clone()).unwrap().unwrap();
    assert_eq!(
        session
            .add_lines(&[(10.0, -179.95), (11.0, 179.95)], true)
            .unwrap(),
        AddLinesOutcome::Added
    );
    session.commit().unwrap();

    let mut expected = antimeridian;
    expected.track_segments.push(TrackSegment {
        track_points: vec![point(10.0, 179.9), point(11.0, -179.9)],
    });
    assert_eq!(
        export_vector(root, "antimeridian-result", &antimeridian_id),
        expected
    );
}

#[test]
fn edit_session_public_behavior() {
    // The application state is process-global, so related scenarios share one
    // test while keeping their assertions in small, named functions.
    let root = initialize_app();
    verify_linked_drawing(root.path());
    verify_box_deletion(root.path());
    verify_antimeridian_snapping(root.path());
}
