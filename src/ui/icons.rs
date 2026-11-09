//! Iconos vectoriales.
//!
//! Se dibujan con primitivas de egui en vez de con glifos de una fuente de
//! iconos por tres motivos: no hay que empaquetar una fuente de 200 KB, no
//! aparecen cuadros de «carácter no encontrado» y el grosor del trazo se adapta
//! al tamaño con el que se pinta, así que a 20 px y a 40 px se ven igual de
//! nítidos.
//!
//! La geometría sigue los iconos de Material que usa YouTube: trazos de 2 px
//! sobre una caja de 24, esquinas redondeadas y sin relleno salvo en los
//! triángulos de reproducción y en los puntos de estado.

use egui::{Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, Ui, Vec2};

/// Iconos disponibles. El nombre describe la forma, no el uso, para poder
/// reutilizarlos en contextos distintos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    // Navegación
    Menu,
    Search,
    Microphone,
    Home,
    HomeFilled,
    Shorts,
    ShortsFilled,
    Subscriptions,
    Library,
    History,
    Clock,
    Playlist,
    WatchLater,
    ThumbsUp,
    ThumbsUpFilled,
    ThumbsDown,
    Videos,
    Downloads,
    Music,
    Films,
    Live,
    Flag,
    Settings,
    Help,
    Feedback,
    // Cabecera
    Create,
    Bell,
    BellFilled,
    Account,
    ChevronDown,
    ChevronRight,
    ChevronLeft,
    ChevronUp,
    ArrowLeft,
    ArrowRight,
    Close,
    More,
    // Reproductor
    Play,
    Pause,
    Next,
    Previous,
    Shuffle,
    Repeat,
    RepeatOne,
    VolumeHigh,
    VolumeMuted,
    Subtitles,
    Quality,
    Theatre,
    Fullscreen,
    FullscreenExit,
    Miniplayer,
    Cast,
    Share,
    Save,
    Remix,
    Comment,
    Sort,
    /// Alias histórico de [`Icon::Playlist`] (la cola del reproductor).
    Queue,
    /// Alias histórico de [`Icon::ThumbsUp`].
    Like,
    /// Alias histórico de [`Icon::ThumbsDown`].
    Dislike,
    // Estados
    Check,
    Plus,
    Trash,
    Link,
    Globe,
    Lock,
    Google,
    Studio,
    Shopping,
    Shield,
    Keyboard,
    Star,
    Spinner,
    Warning,
}

/// Caja de referencia de los iconos (los de Material son de 24).
const BOX: f32 = 24.0;

/// Dibuja un icono centrado en `rect`.
pub fn draw(ui: &Ui, rect: Rect, icon: Icon, color: Color32) {
    let size = rect.width().min(rect.height());
    // Escala del trazo: 2 px sobre una caja de 24.
    let w = (size / BOX * 2.0).max(1.25);
    let s = Stroke::new(w, color);
    let c = rect.center();
    let u = size / BOX; // una unidad de la caja de 24
    let p = ui.painter();

    // Atajos de coordenadas: `d(x, y)` sitúa un punto relativo al centro.
    let d = |x: f32, y: f32| c + Vec2::new(x * u, y * u);
    let line = |a: (f32, f32), b: (f32, f32)| p.line_segment([d(a.0, a.1), d(b.0, b.1)], s);
    let tri = |dir: f32, size: f32| {
        let h = size * u;
        let wd = size * 0.86 * u;
        let mut shape = egui::epaint::PathShape::convex_polygon(
            vec![
                c + Vec2::new(dir * wd * 0.55, 0.0),
                c + Vec2::new(-dir * wd * 0.45, -h * 0.5),
                c + Vec2::new(-dir * wd * 0.45, h * 0.5),
            ],
            color,
            Stroke::NONE,
        );
        shape.fill = color;
        p.add(shape);
    };
    let circle = |x: f32, y: f32, r: f32| p.circle_stroke(d(x, y), r * u, s);
    let disc = |x: f32, y: f32, r: f32| p.circle_filled(d(x, y), r * u, color);
    let rounded = |x0: f32, y0: f32, x1: f32, y1: f32, r: f32| {
        p.rect_stroke(
            Rect::from_min_max(d(x0, y0), d(x1, y1)),
            CornerRadius::same((r * u) as u8),
            s,
            egui::StrokeKind::Middle,
        )
    };

    match icon {
        Icon::Menu => {
            for y in [-6.0f32, 0.0, 6.0] {
                line((-9.0, y), (9.0, y));
            }
        }
        Icon::Search => {
            circle(-1.5, -1.5, 6.4);
            line((3.0, 3.0), (9.0, 9.0));
        }
        Icon::Microphone => {
            rounded(-2.6, -9.0, 2.6, 2.0, 2.6);
            line((-5.0, 0.0), (-5.0, -1.5));
            line((5.0, 0.0), (5.0, -1.5));
            // Arco del micrófono.
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [d(-5.0, -1.5), d(-5.0, 5.5), d(5.0, 5.5), d(5.0, -1.5)],
                false,
                Color32::TRANSPARENT,
                s,
            ));
            line((0.0, 5.5), (0.0, 9.0));
            line((-3.0, 9.0), (3.0, 9.0));
        }
        Icon::Home | Icon::HomeFilled => {
            // Tejado.
            line((-9.0, -1.0), (0.0, -9.0));
            line((9.0, -1.0), (0.0, -9.0));
            line((-7.5, 0.5), (-7.5, 8.5));
            line((7.5, 0.5), (7.5, 8.5));
            line((-7.5, 8.5), (7.5, 8.5));
            if icon == Icon::HomeFilled {
                let mut shape = egui::epaint::PathShape::convex_polygon(
                    vec![
                        d(-7.5, 8.5),
                        d(7.5, 8.5),
                        d(7.5, 0.0),
                        d(0.0, -7.0),
                        d(-7.5, 0.0),
                    ],
                    color,
                    Stroke::NONE,
                );
                shape.fill = color;
                p.add(shape);
            }
        }
        Icon::Shorts => {
            // Rectángulo redondeado girado, con el triángulo dentro.
            let r = Rect::from_center_size(c, Vec2::new(11.0 * u, 18.0 * u));
            p.rect_stroke(
                r,
                CornerRadius::same((5.5 * u) as u8),
                s,
                egui::StrokeKind::Middle,
            );
            tri(1.0, 6.0);
        }
        Icon::ShortsFilled => {
            let r = Rect::from_center_size(c, Vec2::new(12.0 * u, 19.0 * u));
            p.rect_filled(r, CornerRadius::same((6.0 * u) as u8), color);
            let mut shape = egui::epaint::PathShape::convex_polygon(
                vec![d(2.0, 0.0), d(-2.0, -3.6), d(-2.0, 3.6)],
                Color32::BLACK,
                Stroke::NONE,
            );
            shape.fill = Color32::BLACK;
            p.add(shape);
        }
        Icon::Subscriptions => {
            rounded(-9.0, -6.5, 9.0, 4.5, 2.0);
            disc(0.0, 8.0, 1.6);
        }
        Icon::Library => {
            line((-8.0, -8.0), (-8.0, 8.0));
            line((-4.0, -8.0), (-4.0, 8.0));
            line((0.5, -7.0), (4.5, 8.0));
            line((7.0, -6.0), (9.0, 8.0));
        }
        Icon::History => {
            circle(0.0, 0.0, 8.6);
            line((0.0, 0.0), (0.0, -5.0));
            line((0.0, 0.0), (4.0, 2.5));
        }
        Icon::Clock => {
            circle(0.0, 0.0, 8.6);
            line((0.0, 0.0), (0.0, -5.0));
            line((0.0, 0.0), (3.6, 0.0));
        }
        Icon::Queue => {
            for y in [-6.0f32, 0.0, 6.0] {
                line((-9.0, y), (5.0, y));
            }
            tri(1.0, 5.0);
            p.circle_filled(d(7.5, 6.0), 1.2 * u, color);
        }
        Icon::Playlist => {
            for y in [-6.0f32, 0.0, 6.0] {
                line((-9.0, y), (5.0, y));
            }
            tri(1.0, 5.0);
            p.circle_filled(d(7.5, 6.0), 1.2 * u, color);
        }
        Icon::Downloads => {
            line((0.0, -8.0), (0.0, 3.0));
            line((-4.0, -1.0), (0.0, 3.5));
            line((4.0, -1.0), (0.0, 3.5));
            line((-8.0, 8.0), (8.0, 8.0));
        }
        Icon::WatchLater => {
            circle(0.0, 0.0, 8.6);
            line((0.0, 0.0), (0.0, -5.0));
            line((0.0, 0.0), (3.6, 0.0));
        }
        Icon::ThumbsUp | Icon::ThumbsUpFilled => {
            // Muñeca y pulgar.
            p.rect_stroke(
                Rect::from_min_max(d(-9.5, -1.0), d(-5.0, 8.5)),
                CornerRadius::same(1),
                s,
                egui::StrokeKind::Middle,
            );
            let pts = vec![
                d(-5.0, -1.0),
                d(-4.0, -6.0),
                d(-1.0, -9.0),
                d(0.5, -8.2),
                d(0.0, -5.0),
                d(-0.5, -1.0),
                d(6.5, -1.0),
                d(8.5, 0.5),
                d(7.5, 3.0),
                d(8.0, 5.5),
                d(6.5, 8.5),
                d(-5.0, 8.5),
            ];
            if icon == Icon::ThumbsUpFilled {
                let mut shape = egui::epaint::PathShape::convex_polygon(pts, color, Stroke::NONE);
                shape.fill = color;
                p.add(shape);
            } else {
                for i in 0..pts.len() - 1 {
                    p.line_segment([pts[i], pts[i + 1]], s);
                }
            }
        }
        Icon::Like => {
            draw(ui, rect, Icon::ThumbsUp, color);
        }
        Icon::Dislike => {
            draw(ui, rect, Icon::ThumbsDown, color);
        }
        Icon::ThumbsDown => {
            p.rect_stroke(
                Rect::from_min_max(d(-9.5, -8.5), d(-5.0, 1.0)),
                CornerRadius::same(1),
                s,
                egui::StrokeKind::Middle,
            );
            let pts = vec![
                d(-5.0, 1.0),
                d(-4.0, 6.0),
                d(-1.0, 9.0),
                d(0.5, 8.2),
                d(0.0, 5.0),
                d(-0.5, 1.0),
                d(6.5, 1.0),
                d(8.5, -0.5),
                d(7.5, -3.0),
                d(8.0, -5.5),
                d(6.5, -8.5),
                d(-5.0, -8.5),
            ];
            for i in 0..pts.len() - 1 {
                p.line_segment([pts[i], pts[i + 1]], s);
            }
        }
        Icon::Videos => {
            rounded(-9.0, -6.5, 9.0, 6.5, 2.0);
            tri(1.0, 5.0);
        }
        Icon::Music => {
            line((5.0, -9.0), (5.0, 4.0));
            line((-5.0, -7.0), (-5.0, 6.0));
            line((5.0, -9.0), (-5.0, -7.0));
            disc(2.6, 5.0, 2.4);
            disc(-7.4, 7.0, 2.4);
        }
        Icon::Films => {
            rounded(-9.0, -6.5, 9.0, 6.5, 2.0);
            for x in [-4.0f32, 0.5, 5.0] {
                line((x, -6.5), (x, 6.5));
            }
        }
        Icon::Live => {
            circle(-4.0, -6.0, 1.6);
            circle(4.0, -6.0, 1.6);
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [d(-8.0, 0.0), d(-8.0, 8.0), d(-3.0, 8.0), d(-2.0, 4.0)],
                false,
                Color32::TRANSPARENT,
                s,
            ));
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [d(8.0, 0.0), d(8.0, 8.0), d(3.0, 8.0), d(2.0, 4.0)],
                false,
                Color32::TRANSPARENT,
                s,
            ));
        }
        Icon::Flag => {
            line((-6.0, -9.0), (-6.0, 9.0));
            let pts = [d(-6.0, -9.0), d(7.0, -5.0), d(-6.0, -1.0)];
            for i in 0..pts.len() - 1 {
                p.line_segment([pts[i], pts[i + 1]], s);
            }
        }
        Icon::Settings => {
            circle(0.0, 0.0, 3.4);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let dir = Vec2::new(a.cos(), a.sin());
                p.line_segment(
                    [c + dir * 6.2 * u, c + dir * 9.2 * u],
                    Stroke::new(w * 1.5, color),
                );
            }
        }
        Icon::Help => {
            circle(0.0, 0.0, 8.6);
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [d(-2.2, -3.0), d(-2.2, -5.5), d(2.2, -5.5), d(2.2, -2.0)],
                false,
                Color32::TRANSPARENT,
                s,
            ));
            line((0.0, -2.0), (0.0, 1.5));
            disc(0.0, 5.0, 1.1);
        }
        Icon::Feedback => {
            rounded(-9.0, -8.0, 9.0, 4.0, 2.0);
            line((-4.0, 4.0), (-2.5, 8.0));
            line((-2.5, 8.0), (0.0, 4.0));
            line((-3.0, -3.0), (-3.0, -1.0));
            disc(0.0, -2.0, 1.1);
            disc(3.0, -2.0, 1.1);
        }
        Icon::Create => {
            line((-8.0, -4.0), (8.0, -4.0));
            line((0.0, -8.0), (0.0, 0.0));
            line((-8.0, 2.0), (-8.0, 8.0));
            line((8.0, 2.0), (8.0, 8.0));
            line((-8.0, 8.0), (8.0, 8.0));
        }
        Icon::Bell | Icon::BellFilled => {
            let pts = [
                d(-6.5, 4.0),
                d(-6.5, -1.0),
                d(-3.0, -6.5),
                d(3.0, -6.5),
                d(6.5, -1.0),
                d(6.5, 4.0),
                d(8.5, 6.0),
                d(-8.5, 6.0),
                d(-6.5, 4.0),
            ];
            if icon == Icon::BellFilled {
                let mut shape =
                    egui::epaint::PathShape::convex_polygon(pts[..7].to_vec(), color, Stroke::NONE);
                shape.fill = color;
                p.add(shape);
                // Badge de "hay nuevas".
                disc(6.0, -7.0, 2.6);
            } else {
                for i in 0..pts.len() - 1 {
                    p.line_segment([pts[i], pts[i + 1]], s);
                }
            }
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [d(-2.5, 7.5), d(-1.0, 9.5), d(1.0, 9.5), d(2.5, 7.5)],
                false,
                Color32::TRANSPARENT,
                s,
            ));
        }
        Icon::Account => {
            circle(0.0, -4.0, 4.0);
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [d(-8.5, 9.0), d(-8.5, 3.0), d(8.5, 3.0), d(8.5, 9.0)],
                false,
                Color32::TRANSPARENT,
                s,
            ));
        }
        Icon::ChevronDown => {
            line((-6.0, -3.0), (0.0, 3.0));
            line((6.0, -3.0), (0.0, 3.0));
        }
        Icon::ChevronUp => {
            line((-6.0, 3.0), (0.0, -3.0));
            line((6.0, 3.0), (0.0, -3.0));
        }
        Icon::ChevronRight => {
            line((-3.0, -6.0), (3.0, 0.0));
            line((-3.0, 6.0), (3.0, 0.0));
        }
        Icon::ChevronLeft => {
            line((3.0, -6.0), (-3.0, 0.0));
            line((3.0, 6.0), (-3.0, 0.0));
        }
        Icon::ArrowLeft => {
            line((8.0, 0.0), (-8.0, 0.0));
            line((-3.0, -5.0), (-8.0, 0.0));
            line((-3.0, 5.0), (-8.0, 0.0));
        }
        Icon::ArrowRight => {
            line((-8.0, 0.0), (8.0, 0.0));
            line((3.0, -5.0), (8.0, 0.0));
            line((3.0, 5.0), (8.0, 0.0));
        }
        Icon::Close => {
            line((-6.5, -6.5), (6.5, 6.5));
            line((-6.5, 6.5), (6.5, -6.5));
        }
        Icon::More => {
            for x in [-6.0f32, 0.0, 6.0] {
                disc(x, 0.0, 1.5);
            }
        }
        Icon::Play => tri(1.0, 12.0),
        Icon::Pause => {
            let h = 6.0 * u;
            p.rect_filled(
                Rect::from_center_size(d(-3.0, 0.0), Vec2::new(3.0 * u, h * 2.0)),
                CornerRadius::same((u) as u8),
                color,
            );
            p.rect_filled(
                Rect::from_center_size(d(3.0, 0.0), Vec2::new(3.0 * u, h * 2.0)),
                CornerRadius::same((u) as u8),
                color,
            );
        }
        Icon::Next => {
            tri(1.0, 9.0);
            p.rect_filled(
                Rect::from_center_size(d(6.5, 0.0), Vec2::new(2.0 * u, 13.0 * u)),
                CornerRadius::same(1),
                color,
            );
        }
        Icon::Previous => {
            tri(-1.0, 9.0);
            p.rect_filled(
                Rect::from_center_size(d(-6.5, 0.0), Vec2::new(2.0 * u, 13.0 * u)),
                CornerRadius::same(1),
                color,
            );
        }
        Icon::Shuffle => {
            line((-8.0, -5.0), (-3.0, -5.0));
            line((-3.0, -5.0), (4.0, 5.0));
            line((4.0, 5.0), (8.0, 5.0));
            line((-8.0, 5.0), (-3.0, 5.0));
            line((-3.0, 5.0), (4.0, -5.0));
            line((4.0, -5.0), (8.0, -5.0));
            line((6.0, -7.0), (8.0, -5.0));
            line((6.0, 7.0), (8.0, 5.0));
        }
        Icon::Repeat | Icon::RepeatOne => {
            rounded(-9.0, -6.0, 9.0, 6.0, 3.0);
            line((5.0, -6.0), (8.5, -6.0));
            line((8.5, -6.0), (8.5, -2.5));
            line((-5.0, 6.0), (-8.5, 6.0));
            line((-8.5, 6.0), (-8.5, 2.5));
            if icon == Icon::RepeatOne {
                line((0.0, -2.5), (0.0, 2.5));
                line((-1.5, -1.5), (0.0, -2.5));
            }
        }
        Icon::VolumeHigh => {
            p.rect_filled(
                Rect::from_center_size(d(-7.0, 0.0), Vec2::new(4.0 * u, 5.0 * u)),
                CornerRadius::same(1),
                color,
            );
            let mut shape = egui::epaint::PathShape::convex_polygon(
                vec![d(-5.0, -2.5), d(-1.0, -6.0), d(-1.0, 6.0), d(-5.0, 2.5)],
                color,
                Stroke::NONE,
            );
            shape.fill = color;
            p.add(shape);
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [d(1.0, -4.0), d(4.0, 0.0), d(4.0, 0.0), d(1.0, 4.0)],
                false,
                Color32::TRANSPARENT,
                s,
            ));
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [d(2.5, -7.0), d(7.5, 0.0), d(7.5, 0.0), d(2.5, 7.0)],
                false,
                Color32::TRANSPARENT,
                s,
            ));
        }
        Icon::VolumeMuted => {
            p.rect_filled(
                Rect::from_center_size(d(-7.0, 0.0), Vec2::new(4.0 * u, 5.0 * u)),
                CornerRadius::same(1),
                color,
            );
            let mut shape = egui::epaint::PathShape::convex_polygon(
                vec![d(-5.0, -2.5), d(-1.0, -6.0), d(-1.0, 6.0), d(-5.0, 2.5)],
                color,
                Stroke::NONE,
            );
            shape.fill = color;
            p.add(shape);
            line((2.0, -4.0), (8.0, 4.0));
            line((2.0, 4.0), (8.0, -4.0));
        }
        Icon::Subtitles => {
            rounded(-9.0, -6.0, 9.0, 6.0, 2.0);
            line((-6.0, -1.0), (-2.0, -1.0));
            line((0.0, -1.0), (6.0, -1.0));
            line((-6.0, 3.0), (2.0, 3.0));
            line((4.0, 3.0), (6.0, 3.0));
        }
        Icon::Quality => {
            rounded(-9.0, -6.5, 9.0, 6.5, 2.0);
            line((-4.0, 3.5), (-2.0, -3.5));
            line((-2.0, -3.5), (0.0, 1.5));
            line((0.0, 1.5), (2.0, -3.5));
            line((2.0, -3.5), (4.0, 3.5));
        }
        Icon::Theatre => {
            rounded(-9.0, -5.0, 9.0, 5.0, 2.0);
            line((-9.0, -2.0), (9.0, -2.0));
        }
        Icon::Fullscreen => {
            for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                p.line_segment([d(sx * 8.5, sy * 8.5), d(sx * 4.5, sy * 8.5)], s);
                p.line_segment([d(sx * 8.5, sy * 8.5), d(sx * 8.5, sy * 4.5)], s);
            }
        }
        Icon::FullscreenExit => {
            for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                p.line_segment([d(sx * 8.5, sy * 8.5), d(sx * 4.5, sy * 8.5)], s);
                p.line_segment([d(sx * 8.5, sy * 8.5), d(sx * 8.5, sy * 4.5)], s);
                p.line_segment([d(sx * 4.5, sy * 4.5), d(sx * 4.5, sy * 8.5)], s);
                p.line_segment([d(sx * 4.5, sy * 4.5), d(sx * 8.5, sy * 4.5)], s);
            }
        }
        Icon::Miniplayer => {
            rounded(-9.0, -6.5, 9.0, 6.5, 2.0);
            p.rect_filled(
                Rect::from_min_max(d(1.0, 1.0), d(8.0, 5.5)),
                CornerRadius::same(1),
                color,
            );
        }
        Icon::Cast => {
            line((-4.0, -6.5), (8.0, -6.5));
            line((8.0, -6.5), (8.0, 6.0));
            line((-2.0, 6.0), (8.0, 6.0));
            line((-9.0, 8.0), (-6.0, 8.0));
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [d(-8.5, 3.0), d(-3.0, 4.0), d(-2.0, 8.0), d(-2.0, 8.0)],
                false,
                Color32::TRANSPARENT,
                s,
            ));
            disc(-8.0, 7.0, 1.2);
        }
        Icon::Share => {
            line((-6.0, 0.0), (7.0, -6.0));
            line((-6.0, 0.0), (7.0, 6.0));
            disc(-7.0, 0.0, 2.6);
            disc(8.0, -7.0, 2.4);
            disc(8.0, 7.0, 2.4);
        }
        Icon::Save => {
            line((-3.0, -9.0), (3.0, -9.0));
            line((-3.0, 9.0), (3.0, 9.0));
            line((-6.0, -9.0), (0.0, -9.0));
            line((-6.0, 9.0), (0.0, 9.0));
            line((-6.0, -9.0), (-6.0, 9.0));
            line((6.0, -9.0), (6.0, 9.0));
        }
        Icon::Remix => {
            circle(0.0, 0.0, 8.6);
            line((0.0, 0.0), (0.0, -5.0));
            line((0.0, 0.0), (3.6, 0.0));
            line((-4.0, -8.0), (-4.0, -6.0));
        }
        Icon::Comment => {
            let pts = vec![
                d(-8.5, -7.0),
                d(8.5, -7.0),
                d(8.5, 4.0),
                d(0.0, 4.0),
                d(-3.5, 8.0),
                d(-3.5, 4.0),
                d(-8.5, 4.0),
                d(-8.5, -7.0),
            ];
            for i in 0..pts.len() - 1 {
                p.line_segment([pts[i], pts[i + 1]], s);
            }
        }
        Icon::Sort => {
            for y in [-6.0f32, -1.0, 4.0] {
                line((-8.0, y), (2.0 + y, y));
            }
        }
        Icon::Check => {
            line((-6.0, 0.0), (-1.5, 5.0));
            line((-1.5, 5.0), (6.5, -5.0));
        }
        Icon::Plus => {
            line((-7.0, 0.0), (7.0, 0.0));
            line((0.0, -7.0), (0.0, 7.0));
        }
        Icon::Trash => {
            rounded(-6.5, -3.0, 6.5, 8.5, 1.5);
            line((-8.5, -5.0), (8.5, -5.0));
            line((-2.5, -5.0), (-2.5, -8.0));
            line((2.5, -5.0), (2.5, -8.0));
            line((-2.5, -8.0), (2.5, -8.0));
        }
        Icon::Link => {
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [d(-1.0, -6.0), d(3.5, -6.0), d(3.5, 0.0), d(-1.0, 0.0)],
                false,
                Color32::TRANSPARENT,
                s,
            ));
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [d(1.0, 6.0), d(-3.5, 6.0), d(-3.5, 0.0), d(1.0, 0.0)],
                false,
                Color32::TRANSPARENT,
                s,
            ));
            line((-3.0, 3.0), (3.0, 3.0));
        }
        Icon::Globe => {
            circle(0.0, 0.0, 8.6);
            line((-8.6, 0.0), (8.6, 0.0));
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [d(0.0, -8.6), d(-5.0, 0.0), d(-5.0, 0.0), d(0.0, 8.6)],
                false,
                Color32::TRANSPARENT,
                s,
            ));
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [d(0.0, -8.6), d(5.0, 0.0), d(5.0, 0.0), d(0.0, 8.6)],
                false,
                Color32::TRANSPARENT,
                s,
            ));
        }
        Icon::Lock => {
            rounded(-6.5, -1.0, 6.5, 8.5, 2.0);
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [d(-4.0, -1.0), d(-4.0, -6.0), d(4.0, -6.0), d(4.0, -1.0)],
                false,
                Color32::TRANSPARENT,
                s,
            ));
        }
        Icon::Google => {
            // La G de Google se dibuja como arco abierto con la barra inferior,
            // que es lo que la hace reconocible sin cargar su logotipo.
            circle(0.0, 0.0, 8.6);
            line((0.0, 0.0), (8.6, 0.0));
            line((0.0, 0.0), (0.0, 3.0));
        }
        Icon::Studio => {
            circle(0.0, 0.0, 8.6);
            disc(0.0, 0.0, 2.6);
            line((4.0, -4.0), (8.0, -8.0));
        }
        Icon::Shopping => {
            circle(0.0, 0.0, 4.0);
            line((0.0, 4.0), (0.0, 9.0));
            line((-6.0, 9.0), (6.0, 9.0));
            line((-6.0, 9.0), (-6.0, -8.0));
            line((6.0, 9.0), (6.0, -8.0));
        }
        Icon::Shield => {
            let pts = vec![
                d(0.0, -9.0),
                d(7.0, -6.0),
                d(7.0, 1.0),
                d(0.0, 9.0),
                d(-7.0, 1.0),
                d(-7.0, -6.0),
                d(0.0, -9.0),
            ];
            for i in 0..pts.len() - 1 {
                p.line_segment([pts[i], pts[i + 1]], s);
            }
        }
        Icon::Keyboard => {
            rounded(-9.0, -5.5, 9.0, 5.5, 2.0);
            for x in [-5.5f32, -2.0, 1.5, 5.0] {
                line((x, -2.0), (x, -2.0));
                disc(x, -2.0, 0.9);
                disc(x, 1.5, 0.9);
            }
        }
        Icon::Star => {
            let mut pts = Vec::new();
            for i in 0..10 {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let r = if i % 2 == 0 { 9.0 } else { 3.9 };
                pts.push(c + Vec2::new(a.cos() * r * u, a.sin() * r * u));
            }
            let mut shape = egui::epaint::PathShape::convex_polygon(pts, color, Stroke::NONE);
            shape.fill = color;
            p.add(shape);
        }
        Icon::Spinner => {
            // Arc de 270°.
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [d(0.0, -8.6), d(8.6, 0.0), d(8.6, 0.0), d(0.0, 8.6)],
                false,
                Color32::TRANSPARENT,
                s,
            ));
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [d(0.0, 8.6), d(-8.6, 0.0), d(-8.6, 0.0), d(-6.0, -6.0)],
                false,
                Color32::TRANSPARENT,
                s,
            ));
        }
        Icon::Warning => {
            let pts = vec![d(0.0, -9.0), d(9.0, 7.5), d(-9.0, 7.5), d(0.0, -9.0)];
            for i in 0..pts.len() - 1 {
                p.line_segment([pts[i], pts[i + 1]], s);
            }
            line((0.0, -3.0), (0.0, 2.0));
            disc(0.0, 5.0, 1.1);
        }
    }
    let _ = (line_label_placeholder as fn(&Ui, Rect, &str, Color32));
}

/// Marcador para que el `match` no quede sin uso de algunas ayudas; no pinta.
#[allow(dead_code)]
fn line_label_placeholder(_ui: &Ui, _rect: Rect, _text: &str, _color: Color32) {}

/// Botón circular con icono, del tamaño de la web (40 px de zona, 24 de icono).
pub fn icon_button(
    ui: &mut Ui,
    icon: Icon,
    icon_size: f32,
    active: bool,
    tooltip: &str,
) -> Response {
    let p = crate::ui::theme::palette(ui.ctx());
    let side = icon_size + 16.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
    if active {
        ui.painter()
            .circle_filled(rect.center(), side * 0.5, p.hover);
    } else if response.hovered() {
        ui.painter()
            .circle_filled(rect.center(), side * 0.5, p.chip);
    }
    draw(
        ui,
        Rect::from_center_size(rect.center(), Vec2::splat(icon_size)),
        icon,
        if active { p.text } else { p.text },
    );
    if tooltip.is_empty() {
        response
    } else {
        response.on_hover_text(tooltip)
    }
}

/// Botón circular de 48 px de la barra lateral compacta: icono + etiqueta.
pub fn rail_item(ui: &mut Ui, icon: Icon, label: &str, selected: bool) -> Response {
    let p = crate::ui::theme::palette(ui.ctx());
    let side = crate::ui::theme::metrics::SIDEBAR_MINI - 8.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(side, 72.0), Sense::click());
    if selected {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(10), p.chip);
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(10), p.hover);
    }
    draw(
        ui,
        Rect::from_center_size(
            egui::pos2(rect.center().x, rect.top() + 26.0),
            Vec2::splat(24.0),
        ),
        icon,
        p.text,
    );
    ui.painter().text(
        egui::pos2(rect.center().x, rect.top() + 54.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(10.0),
        p.text,
    );
    response
}

/// Marca de duración sobre la miniatura: fondo negro 80 %, texto blanco 12 px,
/// radio 4 y 4 px de separación del borde, como en la referencia.
pub fn duration_badge(ui: &Ui, thumb: Rect, text: &str) {
    if text.is_empty() {
        return;
    }
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        FontId::proportional(crate::ui::theme::font::DURATION),
        Color32::WHITE,
    );
    let size = galley.size() + Vec2::new(8.0, 3.0);
    let rect = Rect::from_min_size(
        egui::pos2(thumb.right() - size.x - 4.0, thumb.bottom() - size.y - 4.0),
        size,
    );
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(4),
        Color32::from_rgba_unmultiplied(0, 0, 0, 204),
    );
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, Color32::WHITE);
}

/// Etiqueta roja sobre la miniatura ("EN VIVO", "Nuevo", "Shorts").
pub fn overlay_badge(ui: &Ui, thumb: Rect, text: &str, live: bool) {
    if text.is_empty() {
        return;
    }
    let p = crate::ui::theme::palette(ui.ctx());
    let galley = ui.painter().layout_no_wrap(
        text.to_uppercase(),
        FontId::proportional(crate::ui::theme::font::BADGE),
        Color32::WHITE,
    );
    let size = galley.size() + Vec2::new(10.0, 4.0);
    let rect = Rect::from_min_size(thumb.left_top() + Vec2::new(4.0, 4.0), size);
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(4),
        if live {
            p.brand
        } else {
            Color32::from_rgba_unmultiplied(0, 0, 0, 204)
        },
    );
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, Color32::WHITE);
}
