// Generates every app and tray icon from build/logo-source.webp.
// Run from the repository root on macOS: swift scripts/generate-icons.swift
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

let source = "build/logo-source.webp"

func fail(_ message: String) -> Never {
  FileHandle.standardError.write(Data("Error: \(message)\n".utf8))
  exit(1)
}

guard
  let imageSource = CGImageSourceCreateWithURL(URL(fileURLWithPath: source) as CFURL, nil),
  let loaded = CGImageSourceCreateImageAtIndex(imageSource, 0, nil)
else { fail("could not read \(source)") }

let width = loaded.width
let height = loaded.height
let sRGB = CGColorSpace(name: CGColorSpace.sRGB)!
let bitmapInfo = CGImageAlphaInfo.premultipliedLast.rawValue

func makeContext(_ size: Int) -> CGContext {
  let context = CGContext(
    data: nil, width: size, height: size, bitsPerComponent: 8, bytesPerRow: 0,
    space: sRGB, bitmapInfo: bitmapInfo)!
  context.interpolationQuality = .high
  return context
}

// Normalize to RGBA so the pixels can be read directly.
let pixelContext = CGContext(
  data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
  space: sRGB, bitmapInfo: bitmapInfo)!
pixelContext.draw(loaded, in: CGRect(x: 0, y: 0, width: width, height: height))
let artwork = pixelContext.makeImage()!
let pixels = pixelContext.data!.assumingMemoryBound(to: UInt8.self)

/// The artwork is saturated up to its edge, while the white background and its soft shadow are pale.
func isArtwork(x: Int, y: Int) -> Bool {
  guard x >= 0, y >= 0, x < width, y < height else { return false }
  let offset = (y * width + x) * 4
  let channels = [pixels[offset], pixels[offset + 1], pixels[offset + 2]]
  return Int(channels.max()!) - Int(channels.min()!) > 90
}

// The source has no alpha channel, so trace the outline along rays from the center.
let center = CGPoint(x: CGFloat(width) / 2, y: CGFloat(height) / 2)
let rayCount = 1440
let edgeInset: CGFloat = 4 // Stays inside the anti-aliased edge, which would leave a pale fringe.
let radii: [CGFloat] = (0..<rayCount).map { index in
  let angle = CGFloat(index) / CGFloat(rayCount) * 2 * .pi
  var radius = CGFloat(max(width, height))
  while radius > 0 {
    let x = Int(center.x + cos(angle) * radius)
    let y = Int(center.y + sin(angle) * radius)
    if isArtwork(x: x, y: y) { return radius }
    radius -= 0.5
  }
  fail("no artwork found in \(source)")
}
let smoothing = 4
let outline: [CGPoint] = (0..<rayCount).map { index in
  let window = (-smoothing...smoothing).map { radii[(index + $0 + rayCount) % rayCount] }
  let radius = window.reduce(0, +) / CGFloat(window.count) - edgeInset
  let angle = CGFloat(index) / CGFloat(rayCount) * 2 * .pi
  // Pixel rows run top-down, while Core Graphics coordinates run bottom-up.
  return CGPoint(x: center.x + cos(angle) * radius, y: CGFloat(height) - (center.y + sin(angle) * radius))
}
let mask = CGMutablePath()
mask.addLines(between: outline)
mask.closeSubpath()
let bounds = mask.boundingBoxOfPath

/// Draws the logo with a transparent background, with its body filling `body` pixels.
func drawLogo(canvas: Int, body: Int, shadow: Bool) -> CGImage {
  let context = makeContext(canvas)
  let unit = CGFloat(canvas) / 1024
  if shadow {
    // Apple's macOS icon grid: a soft shadow that falls below the body.
    context.setShadow(
      offset: CGSize(width: 0, height: -12 * unit), blur: 28 * unit,
      color: CGColor(srgbRed: 0, green: 0, blue: 0, alpha: 0.3))
  }
  context.beginTransparencyLayer(auxiliaryInfo: nil)
  let margin = CGFloat(canvas - body) / 2
  context.translateBy(x: margin, y: margin)
  context.scaleBy(x: CGFloat(body) / bounds.width, y: CGFloat(body) / bounds.height)
  context.translateBy(x: -bounds.minX, y: -bounds.minY)
  context.addPath(mask)
  context.clip()
  context.draw(artwork, in: CGRect(x: 0, y: 0, width: width, height: height))
  context.endTransparencyLayer()
  return context.makeImage()!
}

/// Halves step by step from a large render, which keeps small sizes sharp and free of aliasing.
func logo(size: Int, bodyRatio: CGFloat, shadow: Bool = false) -> CGImage {
  if size >= 512 {
    return drawLogo(canvas: size, body: Int((CGFloat(size) * bodyRatio).rounded()), shadow: shadow)
  }
  let larger = logo(size: size * 2, bodyRatio: bodyRatio, shadow: shadow)
  let context = makeContext(size)
  context.draw(larger, in: CGRect(x: 0, y: 0, width: size, height: size))
  return context.makeImage()!
}

func pngData(_ image: CGImage) -> Data {
  let data = NSMutableData()
  let destination = CGImageDestinationCreateWithData(data, UTType.png.identifier as CFString, 1, nil)!
  CGImageDestinationAddImage(destination, image, nil)
  guard CGImageDestinationFinalize(destination) else { fail("could not encode a PNG") }
  return data as Data
}

func write(_ data: Data, to path: String) {
  do { try data.write(to: URL(fileURLWithPath: path)) } catch { fail("could not write \(path)") }
  print("\(path): \(data.count) bytes")
}

/// An ICO file that holds PNG images, which Windows Vista and later can read.
func icoData(_ images: [(size: Int, png: Data)]) -> Data {
  var data = Data()
  func append16(_ value: Int) { withUnsafeBytes(of: UInt16(value).littleEndian) { data.append(contentsOf: $0) } }
  func append32(_ value: Int) { withUnsafeBytes(of: UInt32(value).littleEndian) { data.append(contentsOf: $0) } }
  append16(0)
  append16(1)
  append16(images.count)
  var offset = 6 + images.count * 16
  for image in images {
    data.append(UInt8(image.size == 256 ? 0 : image.size)) // 0 means 256 in this format.
    data.append(UInt8(image.size == 256 ? 0 : image.size))
    data.append(contentsOf: [0, 0])
    append16(1)
    append16(32)
    append32(image.png.count)
    append32(offset)
    offset += image.png.count
  }
  for image in images { data.append(image.png) }
  return data
}

func writeIco(sizes: [Int], bodyRatio: CGFloat, to path: String) {
  write(icoData(sizes.map { (size: $0, png: pngData(logo(size: $0, bodyRatio: bodyRatio))) }), to: path)
}

// macOS expects the 824 of 1024 body from Apple's icon grid, with a shadow in the margin.
let iconset = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("fluent-\(UUID().uuidString).iconset")
try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: iconset) }
for points in [16, 32, 128, 256, 512] {
  for scale in [1, 2] {
    let name = scale == 1 ? "icon_\(points)x\(points).png" : "icon_\(points)x\(points)@2x.png"
    let png = pngData(logo(size: points * scale, bodyRatio: 824.0 / 1024, shadow: true))
    try png.write(to: iconset.appendingPathComponent(name))
  }
}
let iconutil = Process()
iconutil.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
iconutil.arguments = ["-c", "icns", iconset.path, "-o", "build/icon.icns"]
try iconutil.run()
iconutil.waitUntilExit()
guard iconutil.terminationStatus == 0 else { fail("iconutil could not build icon.icns") }
print("build/icon.icns: \(try Data(contentsOf: URL(fileURLWithPath: "build/icon.icns")).count) bytes")

// Windows draws icons edge to edge, with no shadow.
writeIco(sizes: [16, 24, 32, 48, 64, 128, 256], bodyRatio: 1, to: "build/icon.ico")

// Tray icons fill their canvas, because every pixel counts at this size.
write(pngData(logo(size: 18, bodyRatio: 1)), to: "static/tray.png")
write(pngData(logo(size: 36, bodyRatio: 1)), to: "static/tray@2x.png")
writeIco(sizes: [16, 20, 24, 32, 40, 48], bodyRatio: 1, to: "static/tray.ico")

// A preview for checking the small sizes by eye. It is not used by the app.
if CommandLine.arguments.contains("--preview"), let path = CommandLine.arguments.last {
  let cell = 300
  let context = makeContext(cell * 3)
  let backgrounds: [(CGFloat, Int)] = [(1, 0), (0.12, 1), (0.6, 2)]
  for (gray, column) in backgrounds {
    context.setFillColor(CGColor(srgbRed: gray, green: gray, blue: gray, alpha: 1))
    context.fill(CGRect(x: column * cell, y: 0, width: cell, height: cell * 3))
    context.interpolationQuality = .none
    context.draw(logo(size: 18, bodyRatio: 1), in: CGRect(x: column * cell + 20, y: 20, width: 108, height: 108))
    context.draw(logo(size: 36, bodyRatio: 1), in: CGRect(x: column * cell + 160, y: 20, width: 108, height: 108))
    context.interpolationQuality = .high
    context.draw(logo(size: 512, bodyRatio: 824.0 / 1024, shadow: true), in: CGRect(x: column * cell + 20, y: 160, width: 260, height: 260))
    context.draw(logo(size: 512, bodyRatio: 1), in: CGRect(x: column * cell + 20, y: 500, width: 260, height: 260))
  }
  write(pngData(context.makeImage()!), to: path)
}
