// Bakes the native label font into an SDF atlas: swiftc -O bake_glyphs.swift -o /tmp/bake && /tmp/bake <geist-latin-wght-normal.woff2> <assets dir>
import CoreGraphics
import CoreText
import Foundation
import ImageIO
import UniformTypeIdentifiers

let size: CGFloat = 56
let spread = 8
let ss = 4
let weight: CGFloat = 620
let atlasWidth = 1024

let args = CommandLine.arguments
let fontData = try Data(contentsOf: URL(fileURLWithPath: args[1])) as CFData
let outDir = URL(fileURLWithPath: args[2])
let base = CTFontManagerCreateFontDescriptorFromData(fontData)!
let wghtTag = 0x7767_6874
let features: [[String: Any]] = ["tnum", "cv11", "ss01"].map {
    ["CTFeatureOpenTypeTag": $0, "CTFeatureOpenTypeValue": 1]
}
let desc = CTFontDescriptorCreateCopyWithAttributes(base, [
    kCTFontVariationAttribute: [wghtTag: weight],
    kCTFontFeatureSettingsAttribute: features,
] as CFDictionary)
let font = CTFontCreateWithFontDescriptor(desc, size * CGFloat(ss), nil)

var chars: [UnicodeScalar] = (32...126).map { UnicodeScalar($0)! }
chars.append(UnicodeScalar(0xB7)!)
chars.append(UnicodeScalar(0x2212)!)

struct Glyph { var cp: UInt32; var w: Int; var h: Int; var bx: Double; var top: Double; var adv: Double; var sdf: [UInt8]; var x = 0; var y = 0 }

func edt1d(_ f: [Double]) -> [Double] {
    let n = f.count
    var d = [Double](repeating: 0, count: n), v = [Int](repeating: 0, count: n), z = [Double](repeating: 0, count: n + 1)
    var k = 0
    z[0] = -.infinity; z[1] = .infinity
    for q in 1..<n {
        var s = ((f[q] + Double(q * q)) - (f[v[k]] + Double(v[k] * v[k]))) / Double(2 * q - 2 * v[k])
        while s <= z[k] {
            k -= 1
            s = ((f[q] + Double(q * q)) - (f[v[k]] + Double(v[k] * v[k]))) / Double(2 * q - 2 * v[k])
        }
        k += 1; v[k] = q; z[k] = s; z[k + 1] = .infinity
    }
    k = 0
    for q in 0..<n {
        while z[k + 1] < Double(q) { k += 1 }
        d[q] = Double((q - v[k]) * (q - v[k])) + f[v[k]]
    }
    return d
}

func edt(_ grid: [Bool], _ w: Int, _ h: Int) -> [Double] {
    let inf = 1e20
    var g = grid.map { $0 ? 0.0 : inf }
    for x in 0..<w {
        let col = edt1d((0..<h).map { g[$0 * w + x] })
        for y in 0..<h { g[y * w + x] = col[y] }
    }
    for y in 0..<h {
        let row = edt1d(Array(g[(y * w)..<(y * w + w)]))
        for x in 0..<w { g[y * w + x] = row[x] }
    }
    return g
}

var glyphs: [Glyph] = []
for c in chars {
    let attr = NSAttributedString(string: String(c), attributes: [NSAttributedString.Key(kCTFontAttributeName as String): font, NSAttributedString.Key(kCTForegroundColorFromContextAttributeName as String): true])
    let line = CTLineCreateWithAttributedString(attr)
    let runs = CTLineGetGlyphRuns(line) as! [CTRun]
    if let run = runs.first, CTRunGetGlyphCount(run) > 0 {
        var g = CGGlyph(0)
        CTRunGetGlyphs(run, CFRange(location: 0, length: 1), &g)
        if g == 0 && c.value != 32 { continue }
    }
    let adv = CTLineGetTypographicBounds(line, nil, nil, nil) / Double(ss)
    let b = CTLineGetBoundsWithOptions(line, .useGlyphPathBounds)
    if b.isEmpty || c.value == 32 {
        glyphs.append(Glyph(cp: c.value, w: 0, h: 0, bx: 0, top: 0, adv: adv, sdf: []))
        continue
    }
    let pad = spread * ss
    let x0 = Int(floor(b.minX)) - pad, y0 = Int(floor(b.minY)) - pad
    let x1 = Int(ceil(b.maxX)) + pad, y1 = Int(ceil(b.maxY)) + pad
    var w = x1 - x0, h = y1 - y0
    w = (w + ss - 1) / ss * ss; h = (h + ss - 1) / ss * ss
    var buf = [UInt8](repeating: 0, count: w * h)
    let ctx = CGContext(data: &buf, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w, space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue)!
    ctx.setFillColor(gray: 1, alpha: 1)
    ctx.textPosition = CGPoint(x: -CGFloat(x0), y: -CGFloat(y0))
    CTLineDraw(line, ctx)
    let inside = buf.map { $0 >= 128 }
    let dOut = edt(inside, w, h)
    let dIn = edt(inside.map { !$0 }, w, h)
    let (gw, gh) = (w / ss, h / ss)
    var sdf = [UInt8](repeating: 0, count: gw * gh)
    for gy in 0..<gh {
        for gx in 0..<gw {
            var sum = 0.0
            for sy in 0..<ss { for sx in 0..<ss {
                let i = (gy * ss + sy) * w + gx * ss + sx
                sum += inside[i] ? sqrt(dIn[i]) - 0.5 : -(sqrt(dOut[i]) - 0.5)
            } }
            let d = sum / Double(ss * ss) / Double(ss)
            sdf[gy * gw + gx] = UInt8(max(0, min(255, (0.5 + d / Double(2 * spread)) * 255).rounded()))
        }
    }
    let rows = sdf
    glyphs.append(Glyph(cp: c.value, w: gw, h: gh, bx: Double(x0) / Double(ss), top: Double(y0 + h) / Double(ss), adv: adv, sdf: rows))
}


var (px, py, shelf) = (1, 1, 0)
for i in glyphs.indices where glyphs[i].w > 0 {
    if px + glyphs[i].w + 1 > atlasWidth { px = 1; py += shelf + 1; shelf = 0 }
    glyphs[i].x = px; glyphs[i].y = py
    px += glyphs[i].w + 1
    shelf = max(shelf, glyphs[i].h)
}
let atlasHeight = py + shelf + 1
var atlas = [UInt8](repeating: 0, count: atlasWidth * atlasHeight)
for g in glyphs where g.w > 0 {
    for y in 0..<g.h { for x in 0..<g.w { atlas[(g.y + y) * atlasWidth + g.x + x] = g.sdf[y * g.w + x] } }
}
let provider = CGDataProvider(data: Data(atlas) as CFData)!
let image = CGImage(width: atlasWidth, height: atlasHeight, bitsPerComponent: 8, bitsPerPixel: 8, bytesPerRow: atlasWidth, space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGBitmapInfo(rawValue: 0), provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)!
let dest = CGImageDestinationCreateWithURL(outDir.appendingPathComponent("glyphs.png") as CFURL, UTType.png.identifier as CFString, 1, nil)!
CGImageDestinationAddImage(dest, image, nil)
CGImageDestinationFinalize(dest)

let ascent = CTFontGetAscent(font) / CGFloat(ss), descent = CTFontGetDescent(font) / CGFloat(ss), cap = CTFontGetCapHeight(font) / CGFloat(ss)
var text = "# size spread ascent descent cap_height | codepoint x y w h bearing_x top advance\n"
text += String(format: "font %.0f %d %.3f %.3f %.3f\n", size, spread, ascent, descent, cap)
for g in glyphs {
    text += String(format: "%d %d %d %d %d %.3f %.3f %.3f\n", g.cp, g.x, g.y, g.w, g.h, g.bx, g.top, g.adv)
}
try text.write(to: outDir.appendingPathComponent("glyphs.txt"), atomically: true, encoding: .utf8)
print("atlas \(atlasWidth)x\(atlasHeight), \(glyphs.count) glyphs")
