use bevy::prelude::*;

#[derive(Component, Default, Deref, DerefMut)]
pub struct HeartbeatValue(pub i32);

#[derive(Bundle)]
pub struct HeartbeatBundle {
    #[bundle()]
    pub node: NodeBundle,
    pub value: HeartbeatValue,

    pub(crate) hb: Heartbeat,
}

impl Default for HeartbeatBundle {
    fn default() -> Self {
        let max_samples = 80;

        Self {
            node: NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    width: Val::Px(200.0),
                    height: Val::Px(40.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::FlexEnd,
                    column_gap: Val::Px(1.0),
                    padding: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                background_color: BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                ..default()
            },
            value: HeartbeatValue(0),
            hb: Heartbeat {
                max_samples,
                samples: vec![0.0; max_samples],
                bars: Vec::new(),
            },
        }
    }
}

pub struct HeartbeatUiPlugin;

impl Plugin for HeartbeatUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (heartbeat_init_bars, heartbeat_tick, heartbeat_render).chain());
    }
}

// ===== internal =====

#[derive(Component)]
pub(crate) struct Heartbeat {
    max_samples: usize,
    samples: Vec<f32>,
    bars: Vec<Entity>,
}

fn heartbeat_init_bars(
    mut commands: Commands,
    mut q: Query<(Entity, &mut Heartbeat), Added<Heartbeat>>,
) {
    for (entity, mut hb) in &mut q {
        let mut bars = Vec::with_capacity(hb.max_samples);

        commands.entity(entity).with_children(|p| {
            for _ in 0..hb.max_samples {
                let e = p
                    .spawn(NodeBundle {
                        style: Style {
                            width: Val::Px(2.0),
                            height: Val::Px(1.0),
                            ..default()
                        },
                        background_color: BackgroundColor(Color::srgb(0.2, 1.0, 0.2)),
                        ..default()
                    })
                    .id();
                bars.push(e);
            }
        });

        hb.bars = bars;
    }
}

fn heartbeat_tick(mut q: Query<(&HeartbeatValue, &mut Heartbeat)>) {
    for (v, mut hb) in &mut q {
        if hb.samples.len() >= hb.max_samples {
            hb.samples.remove(0);
        }
        hb.samples.push(v.0 as f32);

        // keep fixed window size (avoid growth if max_samples changes)
        if hb.samples.len() < hb.max_samples {
            let missing = hb.max_samples - hb.samples.len();
            hb.samples.splice(0..0, std::iter::repeat(0.0).take(missing));
        }
    }
}

fn heartbeat_render(
    roots: Query<(&Heartbeat, &Node), With<HeartbeatValue>>,
    mut styles: Query<&mut Style>,
) {
    for (hb, node) in &roots {
        if hb.bars.is_empty() {
            continue;
        }

        // available height inside padding (8px from UiRect::all(4))
        let h = (node.size().y - 8.0).max(1.0);

        // autoscale current window
        let (mut min, mut max) = (f32::INFINITY, f32::NEG_INFINITY);
        for &s in &hb.samples {
            min = min.min(s);
            max = max.max(s);
        }
        if !min.is_finite() || !max.is_finite() {
            continue;
        }
        let denom = (max - min).max(0.001);

        for (i, &s) in hb.samples.iter().enumerate().take(hb.bars.len()) {
            let bar = hb.bars[i];
            if let Ok(mut st) = styles.get_mut(bar) {
                let t = ((s - min) / denom).clamp(0.0, 1.0);
                st.height = Val::Px(1.0 + t * h);
            }
        }
    }
}