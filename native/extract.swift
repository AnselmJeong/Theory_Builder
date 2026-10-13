import Foundation
import PDFKit
let url = URL(fileURLWithPath: CommandLine.arguments[1])
func emit(_ text: String, _ note: String) {
 let data = try! JSONSerialization.data(withJSONObject: ["text": text, "note": note], options: [.sortedKeys])
 FileHandle.standardOutput.write(data)
}
if let pdf = PDFDocument(url: url), !pdf.isLocked {
 var text = "", read = 0, nonempty = 0
 for i in 0..<pdf.pageCount {
  let page = pdf.page(at: i)?.string ?? ""
  if text.utf8.count + page.utf8.count > 2_000_000 { break }
  read += 1
  if !page.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { nonempty += 1; text += "[PDF page \(i+1)]\n\(page)\n\n" }
 }
 emit(text, nonempty == 0 ? "텍스트 없음 · 스캔 PDF는 OCR이 필요합니다" : "PDF 텍스트 추출 · \(read)/\(pdf.pageCount)페이지 확인 · 텍스트 있는 페이지 \(nonempty) · 페이지 표식 포함")
} else { emit("", "PDF를 읽지 못했습니다. 암호화 또는 손상 여부를 확인하세요") }
