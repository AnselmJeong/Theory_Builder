import AppKit
let target = CommandLine.arguments[1]
let image = NSImage(size: NSSize(width: 1024, height: 1024))
image.lockFocus()
NSColor(calibratedRed: 0.10, green: 0.16, blue: 0.24, alpha: 1).setFill()
NSBezierPath(roundedRect: NSRect(x: 56, y: 56, width: 912, height: 912), xRadius: 205, yRadius: 205).fill()
let nodes: [(CGFloat, CGFloat)] = [(275, 710), (510, 800), (755, 650), (750, 335), (480, 228), (260, 405), (512, 510)]
let links = [(0,1),(1,2),(2,3),(3,4),(4,5),(5,0),(0,6),(2,6),(4,6),(5,6)]
NSColor(calibratedRed: 0.34, green: 0.76, blue: 0.72, alpha: 0.55).setStroke()
for (a,b) in links {
 let path = NSBezierPath(); path.lineWidth = 14
 path.move(to: NSPoint(x:nodes[a].0,y:nodes[a].1));path.line(to:NSPoint(x:nodes[b].0,y:nodes[b].1));path.stroke()
}
for (index,node) in nodes.enumerated() {
 let r: CGFloat = index == 6 ? 62 : 34
 NSColor(calibratedRed: index == 6 ? 0.96 : 0.60, green: 0.91, blue: 0.85, alpha: 1).setFill()
 NSBezierPath(ovalIn: NSRect(x:node.0-r,y:node.1-r,width:r*2,height:r*2)).fill()
}
image.unlockFocus()
let bitmap = NSBitmapImageRep(data: image.tiffRepresentation!)!
try bitmap.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: target))
