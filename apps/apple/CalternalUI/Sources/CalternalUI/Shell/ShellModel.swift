import SwiftUI
import Observation

/// What the shell has open. Desktop shows all three regions by default; a phone shows the middle card and slides the others over it.
@MainActor
@Observable
public final class ShellState {
    public var sidebarOpen: Bool
    public var rightOpen: Bool
    public var panel: String
    public var selectedThread: Int?

    public init(sidebarOpen: Bool = true, rightOpen: Bool = true, panel: String = "chat", selectedThread: Int? = nil) {
        (self.sidebarOpen, self.rightOpen, self.panel, self.selectedThread) = (sidebarOpen, rightOpen, panel, selectedThread)
    }
}

/// Prepares the wallpaper and solves the three veils off the main thread, and hands the result to the shell. One instance per shell.
/// Cost: wallpaper preparation about a millisecond on a development Mac (a 320 px working copy), a solve a few milliseconds per surface; both run when
/// the picture, scheme, Blur/Dim/Tint or window size changes, never per frame.
@MainActor
@Observable
final class WallpaperModel {
    private(set) var context = WallpaperContext()
    private(set) var accent: RGBA?
    private var wallpaper: Wallpaper?
    private var wallpaperKey = ""

    /// What the solve depends on; the shell re-runs `refresh` when it changes.
    struct Input: Hashable {
        var palette: String
        var dark: Bool
        var blur: Bool, dim: Bool, tint: Bool
        var width: Int, height: Int
        var sideWidth: Int, rightWidth: Int, inset: Int
        var wide: Bool
    }

    func refresh(_ input: Input, palette: WallpaperFixtures.Palette) async {
        let size = CGSize(width: input.width, height: input.height)
        guard size.width > 0, size.height > 0 else { return }
        let aspect = (Double(input.width) / Double(input.height) * 20).rounded() / 20
        let key = "\(input.palette)@\(aspect)"
        if wallpaperKey != key || wallpaper == nil {
            let made = await Task.detached(priority: .userInitiated) { Wallpaper(id: key, image: WallpaperFixtures.image(palette), aspect: aspect) }.value
            guard !Task.isCancelled else { return }   // a newer input (a resize, a scheme change) superseded this one; never let a stale result overwrite it
            wallpaper = made
            wallpaperKey = key
        }
        guard let w = wallpaper else { return }
        let flags = MaterialSettings(blur: input.blur, dim: input.dim, tint: input.tint)
        let W = Double(input.width), sw = Double(input.sideWidth) / W, rw = Double(input.rightWidth) / W, gap = Double(input.inset) / W
        let regions: (side: CGRect, right: CGRect, card: CGRect) = input.wide
            ? (CGRect(x: 0, y: 0, width: sw, height: 1), CGRect(x: 1 - rw, y: 0, width: rw, height: 1), CGRect(x: sw + gap, y: 0, width: max(0.1, 1 - sw - rw - 2 * gap), height: 1))
            : (CGRect(x: 0, y: 0, width: 1, height: 1), CGRect(x: 0, y: 0, width: 1, height: 1), CGRect(x: 0, y: 0, width: 1, height: 1))
        let dark = input.dark
        let veils = await Task.detached(priority: .userInitiated) { () -> (Veil, Veil, Veil) in
            let ink = VeilInk(dark ? Tokens.dark : Tokens.light), other = VeilInk(dark ? Tokens.light : Tokens.dark)
            func solve(_ r: CGRect, _ role: VeilRole) -> Veil {
                VeilSolver.solve(dark: dark, role: role, flags: flags, pixels: w.pixels(in: r, blurred: flags.blur), ink: ink, other: role == .side ? other : nil)
            }
            return (solve(regions.side, .side), solve(regions.card, .card), solve(regions.right, .side))
        }.value
        guard !Task.isCancelled else { return }
        context = WallpaperContext(wallpaper: w, side: veils.0, card: veils.1, windowSize: size)
        rightVeil = veils.2
        accent = w.accentBase
    }

    private(set) var rightVeil: Veil?
}
