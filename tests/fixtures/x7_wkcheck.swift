// T0.44: play a gateway HLS URL in a real WKWebView, seek to a fraction of the
// duration, and report progress, picture size, audio tracks and text tracks.
// Usage: x7_wkcheck <url> <seek fraction 0..1>
import Cocoa
import WebKit

final class Checker: NSObject {
    let url: String, fraction: Double
    let view: WKWebView
    var ticks = 0, seekedTo = -1.0
    init(url: String, fraction: Double) {
        self.url = url; self.fraction = fraction
        let config = WKWebViewConfiguration()
        config.mediaTypesRequiringUserActionForPlayback = []
        view = WKWebView(frame: NSRect(x: 0, y: 0, width: 640, height: 360), configuration: config)
        super.init()
    }
    func start() {
        // WebKit pauses media in hidden or inactive windows.
        let window = NSWindow(contentRect: NSRect(x: 40, y: 40, width: 640, height: 360), styleMask: [.borderless], backing: .buffered, defer: false)
        window.contentView = view
        window.alphaValue = 0.02
        window.level = .floating
        window.orderFrontRegardless()
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        // Like the app's Local video page: native HLS without crossorigin (text tracks load as blob: URLs).
        view.loadHTMLString("<video id=v src=\"\(url)\" autoplay muted playsinline></video>", baseURL: URL(string: "http://localhost/"))
        Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { timer in
            self.ticks += 1
            let js = "(()=>{const v=document.getElementById('v');if(v.paused&&v.readyState>=2)v.play().catch(()=>{});return JSON.stringify({t:v.currentTime,w:v.videoWidth,h:v.videoHeight,d:v.duration,err:v.error&&v.error.code,ready:v.readyState,audio:v.audioTracks?v.audioTracks.length:-1})})()"
            self.view.evaluateJavaScript(js) { result, _ in
                guard let text = result as? String, let data = text.data(using: .utf8),
                      let row = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }
                let t = row["t"] as? Double ?? 0, d = row["d"] as? Double ?? 0
                if self.seekedTo < 0 && t > 1.5 && d > 0 {
                    self.seekedTo = d * self.fraction
                    self.view.evaluateJavaScript("document.getElementById('v').currentTime=\(self.seekedTo)", completionHandler: nil)
                } else if (self.seekedTo >= 0 && t > self.seekedTo + 1.5) || row["err"] is NSNumber || self.ticks > 80 {
                    print("WKCHECK:" + text.dropLast() + ",\"seek\":\(self.seekedTo)}")
                    timer.invalidate()
                    NSApp.terminate(nil)
                }
            }
        }
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let checker = Checker(url: CommandLine.arguments[1], fraction: Double(CommandLine.arguments[2]) ?? 0.6)
checker.start()
app.run()
