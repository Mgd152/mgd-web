//! Every user-visible sentence of the keymap UI, in Vietnamese (default) and
//! English. One exhaustive `match` per language: a missing string is a
//! compile error.

use super::doc::Kind;
use super::keynames::key_label;
use super::templates::Template;
use super::ui_text::UiText;
use crate::profile::Orientation;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    Vi,
    En,
}

/// Number with a fixed count of decimals; Vietnamese uses a decimal comma.
pub fn fmt_num(v: f64, decimals: usize, lang: Lang) -> String {
    if !v.is_finite() {
        return "–".to_string();
    }
    let mut s = format!("{v:.decimals$}");
    if s.starts_with('-') && s.chars().all(|c| !c.is_ascii_digit() || c == '0') {
        s.remove(0); // "-0,00" -> "0,00"
    }
    match lang {
        Lang::Vi => s.replace('.', ","),
        Lang::En => s,
    }
}

/// A number in a repair line: 2 to 6 decimals (trailing zeros beyond the
/// second dropped, so near-boundary repairs still differ), whole numbers for
/// milliseconds, and scientific form for absurd magnitudes.
fn fmt_repair(v: f64, field: RepairField, lang: Lang) -> String {
    if field == RepairField::ReleaseMs {
        return fmt_num(v, 0, lang);
    }
    if v.is_finite() && v.abs() >= 1e9 {
        let s = format!("{v:e}");
        return if lang == Lang::Vi {
            s.replace('.', ",")
        } else {
            s
        };
    }
    let s = fmt_num(v, 6, lang);
    let sep = if lang == Lang::Vi { ',' } else { '.' };
    match s.find(sep) {
        Some(i) => {
            let mut end = s.len();
            while end > i + 3 && s.as_bytes()[end - 1] == b'0' {
                end -= 1;
            }
            s[..end].to_string()
        }
        None => s,
    }
}

/// Mirrorly shortcut a bound key would shadow while playing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MirrorlyAction {
    Home,
    AppSwitcher,
    Lock,
    VolumeDown,
    VolumeUp,
    Screenshot,
    Record,
    LatencyProbe,
    Stats,
    Fullscreen,
    Rotate,
}

/// Non-blocking advice (never prevents saving or playing).
#[derive(Clone, PartialEq, Debug)]
pub enum Advice {
    ShadowsMirrorly {
        key: String,
        action: MirrorlyAction,
    },
    WindowsKey,
    AltKey,
    LockKey,
    HelperKey,
    NearEdge,
    MoreThanFive {
        count: usize,
    },
    NoLook,
    OverlapsLookAnchor,
    /// `profile` is the orientation of the profile; the picture shows the other.
    OrientationMismatch {
        profile: Orientation,
    },
}

/// Why Play cannot start (section 3.8). Re-exported by the shell later.
#[derive(Clone, PartialEq, Debug)]
pub enum PlayRefusal {
    NoProfile,
    NoticeNotAccepted,
    NotConnected,
    Wifi,
    DisplayMode,
    ScrollBusy,
    Orientation { profile: Orientation },
    BrokenProfile,
}

/// The fix button that goes with a refusal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fix {
    OpenProfileList,
    ReadNotice,
    RotateView,
    ShowHowToFix,
}

impl PlayRefusal {
    pub fn fix(&self) -> Option<Fix> {
        match self {
            PlayRefusal::NoProfile => Some(Fix::OpenProfileList),
            PlayRefusal::NoticeNotAccepted => Some(Fix::ReadNotice),
            PlayRefusal::Orientation { .. } => Some(Fix::RotateView),
            PlayRefusal::BrokenProfile => Some(Fix::ShowHowToFix),
            PlayRefusal::NotConnected
            | PlayRefusal::Wifi
            | PlayRefusal::DisplayMode
            | PlayRefusal::ScrollBusy => None,
        }
    }
}

/// Plain-language finding about a broken profile file (section 6.9).
#[derive(Clone, PartialEq, Debug)]
pub enum ProblemMsg {
    NotReadable,
    NewerVersion,
    BadVersion,
    NameTooLong { text: String, max: usize },
    NameEmpty,
    MouseOnToggle,
    TogglePanicSame,
    TooManyControls { max: usize },
    OutOfScreen { label: String },
    RadiusRange { label: String },
    ReachRange,
    SensitivityRange,
    ReleaseRange,
    UnknownKey { key: String, label: String },
    DuplicateKey { key: String, a: String, b: String },
    SecondLook,
    NotUnderstood,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Msg {
    // key capture (2.4)
    Listening,
    Cancel,
    Swap,
    /// `key` is a SPEC key name; `owner` is the display name of its owner.
    KeyTaken {
        key: String,
        owner: String,
    },
    KeyUnsupported,
    EscAgain,
    MouseNotForToggle,
    // toasts and notices
    IntroToast,
    PlayOn {
        toggle: String,
        panic: String,
    },
    PlayOff,
    PanicDone,
    FiveFingers,
    TestFull,
    Sensitivity(f64),
    SensitivityChanged {
        from: f64,
        to: f64,
    },
    SensitivityRevert,
    HelperKeysShadowed,
    Copied,
    SaveFailed,
    Deleted {
        name: String,
    },
    Undo,
    DraftRecovery {
        name: String,
    },
    SavePrompt,
    // refusals, problems, advice
    Refusal(PlayRefusal),
    Fix(Fix),
    Problem(ProblemMsg),
    Advice(Advice),
    // screens
    FooterRules,
    TestBanner,
    NoPicture,
    OrientationBanner {
        profile: Orientation,
    },
    FingersInfo,
    SensHint,
    EmptyHub,
    CreateFirst,
    TemplateHint,
    TemplateName(Template),
    Recommended,
    OverlayCostNote,
    GuideStep(u8),
    GuideStepEmpty(u8),
    NoticeTitle,
    NoticeBullet(u8),
    NoticeAccept,
    StuckTitle,
    StuckStep(u8),
    // names
    NewProfileName(u32),
    /// Control without a label: kind and number (`n` = 0 means "no number").
    ControlName {
        kind: Kind,
        n: u32,
    },
    SlotToggle,
    SlotPanic,
    /// Captions of buttons, labels and menu items (`ui_text.rs`).
    Ui(UiText),
    /// User data shown as is (profile names, key lists): never translated.
    Verbatim(String),
    /// `repair_ranges` report line: a number was clamped into its range.
    Repaired {
        label: String,
        field: RepairField,
        from: f64,
        to: f64,
    },
}

/// Which number `Msg::Repaired` talks about.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RepairField {
    X,
    Y,
    Radius,
    Reach,
    Sensitivity,
    ReleaseMs,
}

impl RepairField {
    fn text(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (RepairField::X, Lang::Vi) => "vị trí ngang",
            (RepairField::X, Lang::En) => "horizontal position",
            (RepairField::Y, Lang::Vi) => "vị trí dọc",
            (RepairField::Y, Lang::En) => "vertical position",
            (RepairField::Radius, Lang::Vi) => "kích thước vòng",
            (RepairField::Radius, Lang::En) => "ring size",
            (RepairField::Reach, Lang::Vi) => "vùng xoay",
            (RepairField::Reach, Lang::En) => "camera area",
            (RepairField::Sensitivity, Lang::Vi) => "độ nhạy",
            (RepairField::Sensitivity, Lang::En) => "sensitivity",
            (RepairField::ReleaseMs, Lang::Vi) => "thời gian tự nhấc ngón (ms)",
            (RepairField::ReleaseMs, Lang::En) => "auto-lift time (ms)",
        }
    }
}

fn key(name: &str, lang: Lang) -> String {
    key_label(name, lang)
}

impl MirrorlyAction {
    pub fn text(self, lang: Lang) -> &'static str {
        use MirrorlyAction::*;
        match lang {
            Lang::Vi => match self {
                Home => "màn hình chính",
                AppSwitcher => "chuyển ứng dụng",
                Lock => "khoá màn hình",
                VolumeDown => "giảm âm lượng",
                VolumeUp => "tăng âm lượng",
                Screenshot => "chụp màn hình",
                Record => "quay màn hình",
                LatencyProbe => "đo độ trễ",
                Stats => "xem thống kê",
                Fullscreen => "toàn màn hình",
                Rotate => "xoay hình",
            },
            Lang::En => match self {
                Home => "Home",
                AppSwitcher => "app switcher",
                Lock => "screen lock",
                VolumeDown => "volume down",
                VolumeUp => "volume up",
                Screenshot => "screenshot",
                Record => "recording",
                LatencyProbe => "latency probe",
                Stats => "statistics",
                Fullscreen => "fullscreen",
                Rotate => "rotate",
            },
        }
    }
}

fn orient_word(o: Orientation, lang: Lang) -> &'static str {
    match (o, lang) {
        (Orientation::Landscape, Lang::Vi) => "NGANG",
        (Orientation::Portrait, Lang::Vi) => "DỌC",
        (Orientation::Landscape, Lang::En) => "LANDSCAPE",
        (Orientation::Portrait, Lang::En) => "PORTRAIT",
    }
}

fn other(o: Orientation) -> Orientation {
    match o {
        Orientation::Landscape => Orientation::Portrait,
        Orientation::Portrait => Orientation::Landscape,
    }
}

fn mismatch(profile: Orientation, lang: Lang) -> String {
    let (p, s) = (
        orient_word(profile, lang),
        orient_word(other(profile), lang),
    );
    match lang {
        Lang::Vi => format!("Hồ sơ này dành cho màn hình {p}, hình đang {s}."),
        Lang::En => format!("This profile is for a {p} screen, the picture is {s}."),
    }
}

impl Advice {
    pub fn text(&self, lang: Lang) -> String {
        match (self, lang) {
            (Advice::ShadowsMirrorly { key: k, action }, Lang::Vi) => format!(
                "{} cũng là phím tắt {} của Mirrorly; khi đang chơi nó sẽ là phím game.",
                key(k, lang),
                action.text(lang)
            ),
            (Advice::ShadowsMirrorly { key: k, action }, Lang::En) => format!(
                "{} is also Mirrorly's {} shortcut; while playing it is a game key.",
                key(k, lang),
                action.text(lang)
            ),
            (Advice::WindowsKey, Lang::Vi) => "Phím Windows sẽ mở menu Start.".into(),
            (Advice::WindowsKey, Lang::En) => "The Windows key opens the Start menu.".into(),
            (Advice::AltKey, Lang::Vi) => "Alt+F4 sẽ đóng cửa sổ.".into(),
            (Advice::AltKey, Lang::En) => "Alt+F4 closes the window.".into(),
            (Advice::LockKey, Lang::Vi) => "Phím này bật/tắt đèn trên bàn phím mỗi lần bấm.".into(),
            (Advice::LockKey, Lang::En) => "This key toggles a keyboard light on every press.".into(),
            (Advice::HelperKey, Lang::Vi) => "Phím này đang dùng để chỉnh độ nhạy khi chơi. Nếu gán, bạn sẽ không chỉnh nhanh được nữa.".into(),
            (Advice::HelperKey, Lang::En) => "This key adjusts sensitivity while playing. If you bind it, quick adjusting stops working.".into(),
            (Advice::NearEdge, Lang::Vi) => "Gần mép màn hình: iPhone có thể hiểu là vuốt hệ thống.".into(),
            (Advice::NearEdge, Lang::En) => "Near the screen edge: the iPhone may read it as a system swipe.".into(),
            (Advice::MoreThanFive { count }, Lang::Vi) => format!("Có {count} nút nhưng điện thoại chỉ nhận 5 ngón cùng lúc."),
            (Advice::MoreThanFive { count }, Lang::En) => format!("There are {count} controls but the phone accepts only 5 fingers at once."),
            (Advice::NoLook, Lang::Vi) => "Hồ sơ chưa có vùng xoay camera: chuột sẽ không xoay được màn hình.".into(),
            (Advice::NoLook, Lang::En) => "The profile has no camera area: the mouse cannot turn the view.".into(),
            (Advice::OverlapsLookAnchor, Lang::Vi) => "Nút này nằm đè lên tâm vùng xoay camera.".into(),
            (Advice::OverlapsLookAnchor, Lang::En) => "This control sits on top of the camera area's centre.".into(),
            (Advice::OrientationMismatch { profile }, _) => mismatch(*profile, lang),
        }
    }
}

impl PlayRefusal {
    pub fn text(&self, lang: Lang) -> String {
        match (self, lang) {
            (PlayRefusal::NoProfile, Lang::Vi) => "Chưa chọn hồ sơ nào.".into(),
            (PlayRefusal::NoProfile, Lang::En) => "No profile is chosen.".into(),
            (PlayRefusal::NoticeNotAccepted, Lang::Vi) => {
                "Bạn cần đọc lưu ý trước lần chơi đầu.".into()
            }
            (PlayRefusal::NoticeNotAccepted, Lang::En) => {
                "Please read the notice before the first play.".into()
            }
            (PlayRefusal::NotConnected, Lang::Vi) => "Điện thoại chưa kết nối.".into(),
            (PlayRefusal::NotConnected, Lang::En) => "The phone is not connected.".into(),
            (PlayRefusal::Wifi, Lang::Vi) => {
                "Đang nối qua Wi-Fi. Bàn phím chỉ chạy khi cắm cáp USB.".into()
            }
            (PlayRefusal::Wifi, Lang::En) => {
                "Connected over Wi-Fi. The keyboard only works with a USB cable.".into()
            }
            (PlayRefusal::DisplayMode, Lang::Vi) => {
                "Đang dùng iPad làm màn hình phụ. Hãy tắt chế độ đó trước.".into()
            }
            (PlayRefusal::DisplayMode, Lang::En) => {
                "The iPad is used as a second display. Turn that mode off first.".into()
            }
            (PlayRefusal::ScrollBusy, Lang::Vi) => "Đang cuộn dở. Thử lại sau một giây.".into(),
            (PlayRefusal::ScrollBusy, Lang::En) => {
                "A scroll is in progress. Try again in a second.".into()
            }
            (PlayRefusal::Orientation { profile }, _) => mismatch(*profile, lang),
            (PlayRefusal::BrokenProfile, Lang::Vi) => "Hồ sơ này bị lỗi nên chưa dùng được.".into(),
            (PlayRefusal::BrokenProfile, Lang::En) => {
                "This profile is broken and cannot be used yet.".into()
            }
        }
    }
}

impl Fix {
    pub fn text(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Fix::OpenProfileList, Lang::Vi) => "Mở danh sách hồ sơ",
            (Fix::OpenProfileList, Lang::En) => "Open the profile list",
            (Fix::ReadNotice, Lang::Vi) => "Đọc lưu ý",
            (Fix::ReadNotice, Lang::En) => "Read the notice",
            (Fix::RotateView, Lang::Vi) => "Xoay hình (F12)",
            (Fix::RotateView, Lang::En) => "Rotate view (F12)",
            (Fix::ShowHowToFix, Lang::Vi) => "Xem cách sửa",
            (Fix::ShowHowToFix, Lang::En) => "Show how to fix",
        }
    }
}

impl ProblemMsg {
    pub fn text(&self, lang: Lang) -> String {
        match (self, lang) {
            (ProblemMsg::NotReadable, Lang::Vi) => "Tệp này không đọc được (không phải hồ sơ bàn phím). Tệp gốc vẫn còn nguyên.".into(),
            (ProblemMsg::NotReadable, Lang::En) => "This file cannot be read (it is not a keyboard profile). The original file is untouched.".into(),
            (ProblemMsg::NewerVersion, Lang::Vi) => "Hồ sơ này làm bằng bản mới hơn. Hãy cập nhật chương trình.".into(),
            (ProblemMsg::NewerVersion, Lang::En) => "This profile was made by a newer version. Please update the program.".into(),
            (ProblemMsg::BadVersion, Lang::Vi) => "Hồ sơ thiếu số phiên bản hợp lệ.".into(),
            (ProblemMsg::BadVersion, Lang::En) => "The profile has no valid version number.".into(),
            (ProblemMsg::NameTooLong { text, max }, Lang::Vi) => format!("Tên “{text}” quá dài (tối đa {max} ký tự)."),
            (ProblemMsg::NameTooLong { text, max }, Lang::En) => format!("The name “{text}” is too long (at most {max} characters)."),
            (ProblemMsg::NameEmpty, Lang::Vi) => "Tên không được để trống.".into(),
            (ProblemMsg::NameEmpty, Lang::En) => "The name cannot be empty.".into(),
            (ProblemMsg::MouseOnToggle, Lang::Vi) => "Phím bật/tắt và phím dừng phải là phím trên bàn phím.".into(),
            (ProblemMsg::MouseOnToggle, Lang::En) => "The on/off key and the stop key must be keyboard keys.".into(),
            (ProblemMsg::TogglePanicSame, Lang::Vi) => "Phím bật/tắt và phím dừng khẩn cấp đang trùng nhau.".into(),
            (ProblemMsg::TogglePanicSame, Lang::En) => "The on/off key and the emergency stop key are the same.".into(),
            (ProblemMsg::TooManyControls { max }, Lang::Vi) => format!("Hồ sơ có quá nhiều nút (tối đa {max})."),
            (ProblemMsg::TooManyControls { max }, Lang::En) => format!("The profile has too many controls (at most {max})."),
            (ProblemMsg::OutOfScreen { label }, Lang::Vi) => format!("Nút “{label}” nằm ngoài màn hình."),
            (ProblemMsg::OutOfScreen { label }, Lang::En) => format!("Control “{label}” is outside the screen."),
            (ProblemMsg::RadiusRange { label }, Lang::Vi) => format!("Vòng của “{label}” quá to hoặc quá nhỏ."),
            (ProblemMsg::RadiusRange { label }, Lang::En) => format!("The ring of “{label}” is too big or too small."),
            (ProblemMsg::ReachRange, Lang::Vi) => "Vùng xoay camera quá to hoặc quá nhỏ.".into(),
            (ProblemMsg::ReachRange, Lang::En) => "The camera area is too big or too small.".into(),
            (ProblemMsg::SensitivityRange, Lang::Vi) => "Độ nhạy chuột nằm ngoài khoảng cho phép.".into(),
            (ProblemMsg::SensitivityRange, Lang::En) => "The mouse sensitivity is outside the allowed range.".into(),
            (ProblemMsg::ReleaseRange, Lang::Vi) => "Thời gian tự nhấc ngón quá dài (tối đa 60 giây).".into(),
            (ProblemMsg::ReleaseRange, Lang::En) => "The automatic finger-lift time is too long (at most 60 seconds).".into(),
            (ProblemMsg::UnknownKey { key: k, label }, Lang::Vi) => format!("Phím “{k}” chưa được hỗ trợ (ở “{label}”)."),
            (ProblemMsg::UnknownKey { key: k, label }, Lang::En) => format!("Key “{k}” is not supported (in “{label}”)."),
            (ProblemMsg::DuplicateKey { key: k, a, b }, Lang::Vi) => format!("Phím {} bị dùng 2 lần: “{a}” và “{b}”.", key(k, lang)),
            (ProblemMsg::DuplicateKey { key: k, a, b }, Lang::En) => format!("Key {} is used twice: “{a}” and “{b}”.", key(k, lang)),
            (ProblemMsg::SecondLook, Lang::Vi) => "Có hơn một vùng xoay camera. Chỉ được 1.".into(),
            (ProblemMsg::SecondLook, Lang::En) => "There is more than one camera area. Only 1 is allowed.".into(),
            (ProblemMsg::NotUnderstood, Lang::Vi) => "Hồ sơ có phần chương trình không hiểu. Có thể nó được làm bằng công cụ khác.".into(),
            (ProblemMsg::NotUnderstood, Lang::En) => "The profile contains something this program does not understand. It may come from another tool.".into(),
        }
    }
}

fn vi(m: &Msg) -> String {
    let l = Lang::Vi;
    match m {
        Msg::Listening => "Bấm phím hoặc nút chuột…".into(),
        Msg::Cancel => "Huỷ".into(),
        Msg::Swap => "Đổi chỗ".into(),
        Msg::KeyTaken { key: k, owner } => format!("Phím {} đang dùng cho “{owner}”.", key(k, l)),
        Msg::KeyUnsupported => "Phím này chưa dùng được. Hãy chọn phím khác.".into(),
        Msg::EscAgain => "Bấm Esc lần nữa để gán phím Esc, hoặc bấm Huỷ.".into(),
        Msg::MouseNotForToggle => "Phím bật/tắt và phím dừng phải là phím trên bàn phím.".into(),
        Msg::IntroToast => "Mới: chơi bằng bàn phím và chuột. Bấm nút Bàn phím trên thanh công cụ.".into(),
        Msg::PlayOn { toggle, panic } => format!(
            "Bàn phím ĐANG BẬT · {} thoát · {} dừng khẩn cấp · [ ] độ nhạy",
            key(toggle, l),
            key(panic, l)
        ),
        Msg::PlayOff => "Đã tắt bàn phím.".into(),
        Msg::PanicDone => "Đã dừng khẩn cấp. Tất cả ngón tay đã nhấc.".into(),
        Msg::FiveFingers => "Đã đủ 5 ngón: phím vừa bấm bị bỏ qua.".into(),
        Msg::TestFull => "Đủ 5/5: phím này bị bỏ qua.".into(),
        Msg::Sensitivity(v) => format!("Độ nhạy chuột: {}", fmt_num(*v, 2, l)),
        Msg::SensitivityChanged { from, to } => format!(
            "Độ nhạy đã đổi {} → {}.",
            fmt_num(*from, 2, l),
            fmt_num(*to, 2, l)
        ),
        Msg::SensitivityRevert => "Trả về như cũ".into(),
        Msg::HelperKeysShadowed => "Phím [ ] đang dùng trong hồ sơ nên không chỉnh độ nhạy khi chơi được.".into(),
        Msg::Copied => "Đã sao chép. Dán vào Zalo/Messenger để gửi cho bạn bè.".into(),
        Msg::SaveFailed => "Không lưu được hồ sơ. Kiểm tra ổ đĩa rồi thử lại.".into(),
        Msg::Deleted { name } => format!("Đã xoá “{name}”."),
        Msg::Undo => "Hoàn tác".into(),
        Msg::DraftRecovery { name } => format!("Lần trước bạn sửa “{name}” chưa lưu."),
        Msg::SavePrompt => "Lưu thay đổi?".into(),
        Msg::Refusal(r) => r.text(l),
        Msg::Fix(f) => f.text(l).into(),
        Msg::Problem(p) => p.text(l),
        Msg::Advice(a) => a.text(l),
        Msg::FooterRules => "1 phím = 1 lần chạm. Không macro.".into(),
        Msg::TestBanner => "ĐANG THỬ – không gửi gì tới điện thoại".into(),
        Msg::NoPicture => "Chưa có hình điện thoại. Vẫn sửa được, nhưng nên mở game để đặt nút cho đúng chỗ.".into(),
        Msg::OrientationBanner { profile } => mismatch(*profile, l),
        Msg::FingersInfo => "Điện thoại chỉ nhận 5 ngón cùng lúc. Ngón thứ 6 sẽ bị bỏ qua.".into(),
        Msg::SensHint => "Còn tuỳ vào chuột và cài đặt trong game. Hãy thử rồi chỉnh.".into(),
        Msg::EmptyHub => "Chưa có hồ sơ nào.".into(),
        Msg::CreateFirst => "Tạo hồ sơ đầu tiên".into(),
        Msg::TemplateHint => "Vị trí chỉ là tạm. Bạn sẽ kéo từng nút cho trùng với nút trong game.".into(),
        Msg::TemplateName(t) => match t {
            Template::ShooterLandscape => "Bắn súng – màn hình ngang",
            Template::BasicLandscape => "Cơ bản – màn hình ngang",
            Template::BasicPortrait => "Cơ bản – màn hình dọc",
            Template::Empty(_) => "Trống",
        }
        .into(),
        Msg::Recommended => "đề nghị".into(),
        Msg::OverlayCostNote => "Có thể làm hình chậm đi một chút. Chưa đo.".into(),
        Msg::GuideStep(n) => match n {
            1 => "Mở game và vào một trận tập để thấy đủ các nút. Xoay hình cho đúng chiều (F12).",
            2 => "Kéo vòng tròn W A S D trùm lên cần di chuyển trong game. Kéo núm tròn để đổi cỡ cho vừa.",
            3 => "Ô nét đứt là vùng chuột xoay camera. Kéo nó vào nửa phải màn hình, chỗ không có nút.",
            4 => "Kéo nút “Bắn” lên đúng nút bắn trong game. Muốn đổi phím: bấm vào ô phím rồi bấm phím bạn thích.",
            5 => "Làm tương tự với các nút còn lại. Nút nào không cần thì chọn rồi bấm Xoá.",
            _ => "Bấm [Thử], rồi bấm W A S D, rê chuột, bấm chuột trái. Chấm xanh là chỗ ngón tay sẽ chạm. Chưa có gì gửi tới điện thoại.",
        }
        .into(),
        Msg::GuideStepEmpty(n) => match n {
            2 => "Kéo [Cần di chuyển] từ bảng bên vào hình.",
            3 => "Kéo [Xoay camera] từ bảng bên vào hình.",
            _ => "Kéo [Nút] từ bảng bên vào hình.",
        }
        .into(),
        Msg::NoticeTitle => "Trước khi chơi bằng bàn phím và chuột".into(),
        Msg::NoticeBullet(n) => match n {
            1 => "Nhà phát hành game (ví dụ Garena với Free Fire) có thể không cho phép phần mềm bên thứ ba. Chúng tôi chưa kiểm chứng luật hiện hành của họ. Bạn hãy tự đọc điều khoản của game trước.",
            2 => "Không có gì bảo đảm tài khoản an toàn, kể cả tài khoản phụ. Game cũng có thể xếp người chơi bàn phím–chuột vào nhóm ghép trận riêng; chúng tôi không biết và không can thiệp việc đó.",
            3 => "Phần mềm này chỉ đổi 1 phím = 1 lần chạm. Không macro, không tự bắn, không tự ngắm, không giảm giật, không che giấu gì.",
            _ => "Điều khiển nhiều ngón trên iPhone thật chưa được kiểm chứng.",
        }
        .into(),
        Msg::NoticeAccept => "Tôi đã đọc và tự chịu trách nhiệm".into(),
        Msg::StuckTitle => "Điện thoại cứ như bị chạm mãi, hoặc không điều khiển được?".into(),
        Msg::StuckStep(n) => match n {
            1 => "Bấm F8 (dừng khẩn cấp): nhấc hết các ngón tay.",
            2 => "Vẫn kẹt: bấm F3 để khoá màn hình điện thoại, rồi mở khoá lại.",
            _ => "Vẫn chưa hết: rút cáp USB ra và cắm lại.",
        }
        .into(),
        Msg::NewProfileName(n) => format!("Hồ sơ {n}"),
        Msg::ControlName { kind, n } => match (kind, n) {
            (Kind::Button, n) => format!("Nút {n}"),
            (Kind::Joystick, 0) => "Cần di chuyển".into(),
            (Kind::Joystick, n) => format!("Cần di chuyển {n}"),
            (Kind::Look, _) => "Xoay camera".into(),
        },
        Msg::SlotToggle => "Phím bật/tắt".into(),
        Msg::SlotPanic => "Phím dừng khẩn cấp".into(),
        Msg::Ui(t) => t.text(l),
        Msg::Verbatim(s) => s.clone(),
        Msg::Repaired { label, field, from, to } => format!(
            "Đã chỉnh {} của “{label}”: {} → {}.",
            field.text(l),
            fmt_repair(*from, *field, l),
            fmt_repair(*to, *field, l)
        ),
    }
}

fn en(m: &Msg) -> String {
    let l = Lang::En;
    match m {
        Msg::Listening => "Press a key or mouse button…".into(),
        Msg::Cancel => "Cancel".into(),
        Msg::Swap => "Swap".into(),
        Msg::KeyTaken { key: k, owner } => format!("Key {} is used for “{owner}”.", key(k, l)),
        Msg::KeyUnsupported => "This key cannot be used yet. Choose another key.".into(),
        Msg::EscAgain => "Press Esc again to bind the Esc key, or press Cancel.".into(),
        Msg::MouseNotForToggle => "The on/off key and the stop key must be keyboard keys.".into(),
        Msg::IntroToast => "New: play with keyboard and mouse. Press the Keyboard button on the toolbar.".into(),
        Msg::PlayOn { toggle, panic } => format!(
            "Keyboard ON · {} exit · {} emergency stop · [ ] sensitivity",
            key(toggle, l),
            key(panic, l)
        ),
        Msg::PlayOff => "Keyboard is off.".into(),
        Msg::PanicDone => "Emergency stop. All fingers lifted.".into(),
        Msg::FiveFingers => "All 5 fingers are down: the key just pressed was ignored.".into(),
        Msg::TestFull => "5/5 reached: this key is ignored.".into(),
        Msg::Sensitivity(v) => format!("Mouse sensitivity: {}", fmt_num(*v, 2, l)),
        Msg::SensitivityChanged { from, to } => format!(
            "Sensitivity changed {} → {}.",
            fmt_num(*from, 2, l),
            fmt_num(*to, 2, l)
        ),
        Msg::SensitivityRevert => "Put it back".into(),
        Msg::HelperKeysShadowed => "The keys [ ] are used by the profile, so sensitivity cannot be adjusted while playing.".into(),
        Msg::Copied => "Copied. Paste it into a chat app to send it to friends.".into(),
        Msg::SaveFailed => "The profile could not be saved. Check the disk and try again.".into(),
        Msg::Deleted { name } => format!("Deleted “{name}”."),
        Msg::Undo => "Undo".into(),
        Msg::DraftRecovery { name } => format!("Last time you edited “{name}” without saving."),
        Msg::SavePrompt => "Save changes?".into(),
        Msg::Refusal(r) => r.text(l),
        Msg::Fix(f) => f.text(l).into(),
        Msg::Problem(p) => p.text(l),
        Msg::Advice(a) => a.text(l),
        Msg::FooterRules => "1 key = 1 touch. No macros.".into(),
        Msg::TestBanner => "TESTING – nothing is sent to the phone".into(),
        Msg::NoPicture => "No phone picture yet. You can still edit, but open the game to place controls correctly.".into(),
        Msg::OrientationBanner { profile } => mismatch(*profile, l),
        Msg::FingersInfo => "The phone accepts only 5 fingers at once. The 6th finger is ignored.".into(),
        Msg::SensHint => "It depends on the mouse and the game's own setting. Try it, then adjust.".into(),
        Msg::EmptyHub => "There are no profiles yet.".into(),
        Msg::CreateFirst => "Create your first profile".into(),
        Msg::TemplateHint => "Positions are only a guess. Drag each button onto the matching button in the game.".into(),
        Msg::TemplateName(t) => match t {
            Template::ShooterLandscape => "Shooter – landscape",
            Template::BasicLandscape => "Basic – landscape",
            Template::BasicPortrait => "Basic – portrait",
            Template::Empty(_) => "Empty",
        }
        .into(),
        Msg::Recommended => "suggested".into(),
        Msg::OverlayCostNote => "May slow the picture down a little. Not measured yet.".into(),
        Msg::GuideStep(n) => match n {
            1 => "Open the game and enter a practice match to see all buttons. Rotate the picture the right way (F12).",
            2 => "Drag the W A S D circle onto the game's movement stick. Drag the round handle to fit its size.",
            3 => "The dashed box is the mouse camera area. Drag it to the right half of the screen where there are no buttons.",
            4 => "Drag the “Fire” button onto the game's fire button. To change its key: click the key cap, then press the key you like.",
            5 => "Do the same for the other buttons. Select any button you do not need and press Delete.",
            _ => "Press [Test], then press W A S D, move the mouse, click the left button. Blue dots show where fingers would touch. Nothing is sent to the phone.",
        }
        .into(),
        Msg::GuideStepEmpty(n) => match n {
            2 => "Drag [Movement stick] from the side panel into the picture.",
            3 => "Drag [Camera] from the side panel into the picture.",
            _ => "Drag [Button] from the side panel into the picture.",
        }
        .into(),
        Msg::NoticeTitle => "Before playing with keyboard and mouse".into(),
        Msg::NoticeBullet(n) => match n {
            1 => "Game publishers (for example Garena with Free Fire) may not allow third-party software. We have not verified their current rules. Please read the game's terms yourself first.",
            2 => "Nothing guarantees that an account is safe, including a spare account. A game may also place keyboard-and-mouse players in a separate matchmaking pool; we do not know and do not interfere with that.",
            3 => "This software only turns 1 key into 1 touch. No macros, no auto-fire, no auto-aim, no recoil control, nothing hidden.",
            _ => "Multi-finger control on a real iPhone has not been verified.",
        }
        .into(),
        Msg::NoticeAccept => "I have read this and take responsibility".into(),
        Msg::StuckTitle => "Does the phone act as if touched all the time, or not respond?".into(),
        Msg::StuckStep(n) => match n {
            1 => "Press F8 (emergency stop): it lifts every finger.",
            2 => "Still stuck: press F3 to lock the phone screen, then unlock it.",
            _ => "Still not fixed: unplug the USB cable and plug it in again.",
        }
        .into(),
        Msg::NewProfileName(n) => format!("Profile {n}"),
        Msg::ControlName { kind, n } => match (kind, n) {
            (Kind::Button, n) => format!("Button {n}"),
            (Kind::Joystick, 0) => "Movement stick".into(),
            (Kind::Joystick, n) => format!("Movement stick {n}"),
            (Kind::Look, _) => "Camera".into(),
        },
        Msg::SlotToggle => "On/off key".into(),
        Msg::SlotPanic => "Emergency stop key".into(),
        Msg::Ui(t) => t.text(l),
        Msg::Verbatim(s) => s.clone(),
        Msg::Repaired { label, field, from, to } => format!(
            "Adjusted {} of “{label}”: {} → {}.",
            field.text(l),
            fmt_repair(*from, *field, l),
            fmt_repair(*to, *field, l)
        ),
    }
}

impl Msg {
    pub fn text(&self, lang: Lang) -> String {
        match lang {
            Lang::Vi => vi(self),
            Lang::En => en(self),
        }
    }

    /// Maximum characters this message may have in either language (UI
    /// budget: toasts and cards must fit without scrolling).
    pub fn budget(&self) -> usize {
        match self {
            Msg::NoticeBullet(_) => 260,
            Msg::GuideStep(_) | Msg::GuideStepEmpty(_) => 200,
            Msg::Advice(_)
            | Msg::Problem(_)
            | Msg::NoPicture
            | Msg::TemplateHint
            | Msg::IntroToast
            | Msg::HelperKeysShadowed => 150,
            _ => 110,
        }
    }

    /// One sample of every variant (and of every parameter shape that changes
    /// the text), for tests in this and later modules.
    pub fn all_samples() -> Vec<Msg> {
        use Orientation::{Landscape, Portrait};
        let mut v = vec![
            Msg::Listening,
            Msg::Cancel,
            Msg::Swap,
            Msg::KeyTaken {
                key: "KeyR".into(),
                owner: "Nạp đạn".into(),
            },
            Msg::KeyUnsupported,
            Msg::EscAgain,
            Msg::MouseNotForToggle,
            Msg::IntroToast,
            Msg::PlayOn {
                toggle: "Backquote".into(),
                panic: "F8".into(),
            },
            Msg::PlayOff,
            Msg::PanicDone,
            Msg::FiveFingers,
            Msg::TestFull,
            Msg::Sensitivity(1.21),
            Msg::SensitivityChanged {
                from: 1.0,
                to: 1.21,
            },
            Msg::SensitivityRevert,
            Msg::HelperKeysShadowed,
            Msg::Copied,
            Msg::SaveFailed,
            Msg::Deleted {
                name: "Hồ sơ 2".into(),
            },
            Msg::Undo,
            Msg::DraftRecovery {
                name: "Hồ sơ 1".into(),
            },
            Msg::SavePrompt,
            Msg::Fix(Fix::OpenProfileList),
            Msg::Fix(Fix::ReadNotice),
            Msg::Fix(Fix::RotateView),
            Msg::Fix(Fix::ShowHowToFix),
            Msg::FooterRules,
            Msg::TestBanner,
            Msg::NoPicture,
            Msg::OrientationBanner { profile: Landscape },
            Msg::OrientationBanner { profile: Portrait },
            Msg::FingersInfo,
            Msg::SensHint,
            Msg::EmptyHub,
            Msg::CreateFirst,
            Msg::TemplateHint,
            Msg::TemplateName(Template::ShooterLandscape),
            Msg::TemplateName(Template::BasicLandscape),
            Msg::TemplateName(Template::BasicPortrait),
            Msg::TemplateName(Template::Empty(Landscape)),
            Msg::Recommended,
            Msg::OverlayCostNote,
            Msg::NoticeTitle,
            Msg::NoticeAccept,
            Msg::StuckTitle,
            Msg::NewProfileName(3),
            Msg::ControlName {
                kind: Kind::Button,
                n: 3,
            },
            Msg::ControlName {
                kind: Kind::Joystick,
                n: 0,
            },
            Msg::ControlName {
                kind: Kind::Joystick,
                n: 2,
            },
            Msg::ControlName {
                kind: Kind::Look,
                n: 0,
            },
            Msg::SlotToggle,
            Msg::SlotPanic,
            Msg::Verbatim("Hồ sơ 1".into()),
            Msg::Repaired {
                label: "Nút 1".into(),
                field: RepairField::Radius,
                from: 0.0,
                to: 0.03,
            },
            Msg::Repaired {
                label: "Camera".into(),
                field: RepairField::ReleaseMs,
                from: 90000.0,
                to: 60000.0,
            },
        ];
        for r in [
            PlayRefusal::NoProfile,
            PlayRefusal::NoticeNotAccepted,
            PlayRefusal::NotConnected,
            PlayRefusal::Wifi,
            PlayRefusal::DisplayMode,
            PlayRefusal::ScrollBusy,
            PlayRefusal::Orientation { profile: Landscape },
            PlayRefusal::Orientation { profile: Portrait },
            PlayRefusal::BrokenProfile,
        ] {
            v.push(Msg::Refusal(r));
        }
        let (a, b) = ("Bắn".to_string(), "Nhảy".to_string());
        for p in [
            ProblemMsg::NotReadable,
            ProblemMsg::NewerVersion,
            ProblemMsg::BadVersion,
            ProblemMsg::NameTooLong {
                text: a.clone(),
                max: 100,
            },
            ProblemMsg::NameEmpty,
            ProblemMsg::MouseOnToggle,
            ProblemMsg::TogglePanicSame,
            ProblemMsg::TooManyControls { max: 64 },
            ProblemMsg::OutOfScreen { label: a.clone() },
            ProblemMsg::RadiusRange { label: a.clone() },
            ProblemMsg::ReachRange,
            ProblemMsg::SensitivityRange,
            ProblemMsg::ReleaseRange,
            ProblemMsg::UnknownKey {
                key: "KeyÀ".into(),
                label: a.clone(),
            },
            ProblemMsg::DuplicateKey {
                key: "KeyW".into(),
                a: a.clone(),
                b,
            },
            ProblemMsg::SecondLook,
            ProblemMsg::NotUnderstood,
        ] {
            v.push(Msg::Problem(p));
        }
        for action in [
            MirrorlyAction::Home,
            MirrorlyAction::AppSwitcher,
            MirrorlyAction::Lock,
            MirrorlyAction::VolumeDown,
            MirrorlyAction::VolumeUp,
            MirrorlyAction::Screenshot,
            MirrorlyAction::Record,
            MirrorlyAction::LatencyProbe,
            MirrorlyAction::Stats,
            MirrorlyAction::Fullscreen,
            MirrorlyAction::Rotate,
        ] {
            v.push(Msg::Advice(Advice::ShadowsMirrorly {
                key: "F4".into(),
                action,
            }));
        }
        for ad in [
            Advice::WindowsKey,
            Advice::AltKey,
            Advice::LockKey,
            Advice::HelperKey,
            Advice::NearEdge,
            Advice::MoreThanFive { count: 7 },
            Advice::NoLook,
            Advice::OverlapsLookAnchor,
            Advice::OrientationMismatch { profile: Landscape },
            Advice::OrientationMismatch { profile: Portrait },
        ] {
            v.push(Msg::Advice(ad));
        }
        v.extend(UiText::all_samples().into_iter().map(Msg::Ui));
        for n in 1..=6 {
            v.push(Msg::GuideStep(n));
        }
        for n in 2..=4 {
            v.push(Msg::GuideStepEmpty(n));
        }
        for n in 1..=4 {
            v.push(Msg::NoticeBullet(n));
        }
        for n in 1..=3 {
            v.push(Msg::StuckStep(n));
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Exhaustive on purpose: a new variant must be added here and to
    /// `all_samples` (the test below checks every tag is sampled).
    fn tag(m: &Msg) -> usize {
        match m {
            Msg::Listening => 0,
            Msg::Cancel => 1,
            Msg::Swap => 2,
            Msg::KeyTaken { .. } => 3,
            Msg::KeyUnsupported => 4,
            Msg::EscAgain => 5,
            Msg::MouseNotForToggle => 6,
            Msg::IntroToast => 7,
            Msg::PlayOn { .. } => 8,
            Msg::PlayOff => 9,
            Msg::PanicDone => 10,
            Msg::FiveFingers => 11,
            Msg::TestFull => 12,
            Msg::Sensitivity(_) => 13,
            Msg::SensitivityChanged { .. } => 14,
            Msg::SensitivityRevert => 15,
            Msg::HelperKeysShadowed => 16,
            Msg::Copied => 17,
            Msg::SaveFailed => 18,
            Msg::Deleted { .. } => 19,
            Msg::Undo => 20,
            Msg::DraftRecovery { .. } => 21,
            Msg::SavePrompt => 22,
            Msg::Refusal(_) => 23,
            Msg::Fix(_) => 24,
            Msg::Problem(_) => 25,
            Msg::Advice(_) => 26,
            Msg::FooterRules => 27,
            Msg::TestBanner => 28,
            Msg::NoPicture => 29,
            Msg::OrientationBanner { .. } => 30,
            Msg::FingersInfo => 31,
            Msg::SensHint => 32,
            Msg::EmptyHub => 33,
            Msg::CreateFirst => 34,
            Msg::TemplateHint => 35,
            Msg::TemplateName(_) => 36,
            Msg::Recommended => 37,
            Msg::OverlayCostNote => 38,
            Msg::GuideStep(_) => 39,
            Msg::GuideStepEmpty(_) => 40,
            Msg::NoticeTitle => 41,
            Msg::NoticeBullet(_) => 42,
            Msg::NoticeAccept => 43,
            Msg::StuckTitle => 44,
            Msg::StuckStep(_) => 45,
            Msg::NewProfileName(_) => 46,
            Msg::ControlName { .. } => 47,
            Msg::SlotToggle => 48,
            Msg::SlotPanic => 49,
            Msg::Repaired { .. } => 50,
            Msg::Ui(_) => 51,
            Msg::Verbatim(_) => 52,
        }
    }
    const VARIANTS: usize = 53;

    #[test]
    fn samples_cover_every_variant() {
        let tags: HashSet<usize> = Msg::all_samples().iter().map(tag).collect();
        assert_eq!(tags.len(), VARIANTS);
        assert!(tags.iter().all(|t| *t < VARIANTS));
    }

    // test 45
    #[test]
    fn every_message_is_non_empty_in_both_languages() {
        for m in Msg::all_samples() {
            for lang in [Lang::Vi, Lang::En] {
                let s = m.text(lang);
                assert!(!s.trim().is_empty(), "{m:?} {lang:?}");
                assert!(!s.contains("{"), "unfilled placeholder: {s}");
            }
            if !matches!(m, Msg::Verbatim(_)) {
                assert_ne!(m.text(Lang::Vi), m.text(Lang::En), "{m:?} not translated");
            }
        }
    }

    // test 46
    #[test]
    fn character_budgets() {
        for m in Msg::all_samples() {
            for lang in [Lang::Vi, Lang::En] {
                let s = m.text(lang);
                let n = s.chars().count();
                assert!(n <= m.budget(), "{m:?} {lang:?}: {n} chars: {s}");
            }
        }
    }

    #[test]
    fn vietnamese_is_default_and_has_diacritics() {
        assert_eq!(Lang::default(), Lang::Vi);
        assert_eq!(Msg::Cancel.text(Lang::Vi), "Huỷ");
        assert_eq!(
            Msg::KeyTaken {
                key: "KeyR".into(),
                owner: "Nạp đạn".into()
            }
            .text(Lang::Vi),
            "Phím R đang dùng cho “Nạp đạn”."
        );
        assert_eq!(Msg::Sensitivity(1.21).text(Lang::Vi), "Độ nhạy chuột: 1,21");
        assert_eq!(
            Msg::FooterRules.text(Lang::Vi),
            "1 phím = 1 lần chạm. Không macro."
        );
        assert_eq!(
            Msg::Advice(Advice::ShadowsMirrorly {
                key: "F4".into(),
                action: MirrorlyAction::VolumeDown
            })
            .text(Lang::Vi),
            "F4 cũng là phím tắt giảm âm lượng của Mirrorly; khi đang chơi nó sẽ là phím game."
        );
    }

    #[test]
    fn refusal_fixes() {
        assert_eq!(PlayRefusal::Wifi.fix(), None);
        assert_eq!(PlayRefusal::BrokenProfile.fix(), Some(Fix::ShowHowToFix));
        assert_eq!(
            PlayRefusal::Orientation {
                profile: Orientation::Landscape
            }
            .text(Lang::Vi),
            "Hồ sơ này dành cho màn hình NGANG, hình đang DỌC."
        );
    }

    // test 51
    #[test]
    fn fmt_num_uses_decimal_comma_in_vietnamese() {
        assert_eq!(fmt_num(1.21, 2, Lang::Vi), "1,21");
        assert_eq!(fmt_num(1.21, 2, Lang::En), "1.21");
        assert_eq!(fmt_num(1.0, 2, Lang::Vi), "1,00");
        assert_eq!(fmt_num(0.6, 1, Lang::Vi), "0,6");
        assert_eq!(fmt_num(10.0, 0, Lang::Vi), "10");
        assert_eq!(fmt_num(-0.001, 2, Lang::Vi), "0,00");
        assert_eq!(fmt_num(-1.5, 1, Lang::Vi), "-1,5");
        assert_eq!(fmt_num(f64::NAN, 2, Lang::Vi), "–");
    }
}
