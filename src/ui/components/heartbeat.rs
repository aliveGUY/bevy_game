use bevy::prelude::*;

#[derive(Component, Default, Deref, DerefMut)]
pub struct HeartbeatValue(pub f32);

#[derive(Bundle)]
pub struct HeartbeatBundle {
    #[bundle()]
    pub node: NodeBundle,
    pub value: HeartbeatValue,
    pub(crate) hb: Heartbeat,
}

impl Default for HeartbeatBundle {
    fn default() -> Self {
        let max_samples = 120; // more samples = more detailed history

        Self {
            node: NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    width: Val::Px(240.0),
                    height: Val::Px(50.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::FlexEnd,
                    column_gap: Val::Px(1.0),
                    padding: UiRect::all(Val::Px(4.0)),
                    ..default()
                },
                background_color: BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                ..default()
            },
            value: HeartbeatValue(0.0),
            hb: Heartbeat {
                max_samples,
                samples: vec![0.0; max_samples],
                bars: Vec::new(),

                // smoothing & scaling
                ema: 0.0,
                ema_alpha: 0.25, // 0..1 (higher = snappier, lower = smoother)
                peak: 0.0,
                peak_fall_per_s: 6.0, // how fast peak drops (units/s)
                scale_min: 0.0,
                scale_max: 1.0,
                scale_lerp: 0.12, // 0..1 (higher = scale adapts faster)

                // visuals
                bar_width_px: 2.0,
                min_bar_px: 1.0,
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

    // smoothing & scaling
    ema: f32,
    ema_alpha: f32,
    peak: f32,
    peak_fall_per_s: f32,
    scale_min: f32,
    scale_max: f32,
    scale_lerp: f32,

    // visuals
    bar_width_px: f32,
    min_bar_px: f32,
}

fn heartbeat_init_bars(
    mut commands: Commands,
    mut q: Query<(Entity, &mut Heartbeat), Added<Heartbeat>>
) {
    for (entity, mut hb) in &mut q {
        let max_samples = hb.max_samples;
        let bar_width_px = hb.bar_width_px;
        let min_bar_px = hb.min_bar_px;

        let mut bars = Vec::with_capacity(max_samples);

        commands.entity(entity).with_children(|p| {
            for _ in 0..max_samples {
                let e = p
                    .spawn(NodeBundle {
                        style: Style {
                            width: Val::Px(bar_width_px),
                            height: Val::Px(min_bar_px),
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

fn heartbeat_tick(time: Res<Time>, mut q: Query<(&HeartbeatValue, &mut Heartbeat)>) {
    let dt = time.delta_seconds();

    for (v, mut hb) in &mut q {
        // EMA smoothing
        if hb.samples.is_empty() {
            hb.ema = v.0;
        } else {
            hb.ema = hb.ema + (v.0 - hb.ema) * hb.ema_alpha;
        }

        // Peak hold
        hb.peak = hb.peak.max(hb.ema);
        hb.peak = (hb.peak - hb.peak_fall_per_s * dt).max(hb.ema);

        // Push sample into fixed window
        if hb.samples.len() >= hb.max_samples {
            hb.samples.remove(0);
        }

        let ema = hb.ema; // <-- local avoids E0502
        hb.samples.push(ema);

        // keep fixed window size (avoid growth if max_samples changes)
        if hb.samples.len() < hb.max_samples {
            let missing = hb.max_samples - hb.samples.len();
            hb.samples.splice(0..0, std::iter::repeat(0.0).take(missing));
        }

        // Soft autoscale
        let (mut wmin, mut wmax) = (f32::INFINITY, f32::NEG_INFINITY);
        for &s in &hb.samples {
            wmin = wmin.min(s);
            wmax = wmax.max(s);
        }

        if wmin.is_finite() && wmax.is_finite() {
            let pad = (wmax - wmin).max(0.001) * 0.08;
            wmin -= pad;
            wmax += pad;

            let lerp_t = hb.scale_lerp;
            hb.scale_min = hb.scale_min + (wmin - hb.scale_min) * lerp_t;
            hb.scale_max = hb.scale_max + (wmax - hb.scale_max) * lerp_t;
        }
    }
}

fn heartbeat_render(
    roots: Query<(&Heartbeat, &Node), With<HeartbeatValue>>,
    mut styles: Query<&mut Style>,
    mut colors: Query<&mut BackgroundColor>
) {
    for (hb, node) in &roots {
        if hb.bars.is_empty() {
            continue;
        }

        // available height inside padding (8px from UiRect::all(4))
        let h = (node.size().y - 8.0).max(1.0);

        let min = hb.scale_min;
        let max = hb.scale_max;
        let denom = (max - min).max(0.001);

        // peak threshold: top ~10% of the current scale
        let peak_threshold = max - 0.1 * denom;

        for (i, &s) in hb.samples.iter().enumerate().take(hb.bars.len()) {
            let bar = hb.bars[i];

            let t = ((s - min) / denom).clamp(0.0, 1.0);
            let bar_h = hb.min_bar_px + t * h;

            if let Ok(mut st) = styles.get_mut(bar) {
                st.height = Val::Px(bar_h);
            }

            // Make peaks brighter / more opaque
            if let Ok(mut bg) = colors.get_mut(bar) {
                if s >= peak_threshold {
                    bg.0 = Color::srgb(0.4, 1.0, 0.4);
                } else {
                    bg.0 = Color::srgb(0.2, 1.0, 0.2);
                }
            }
        }
    }
}