// T0.43 spike: play an fMP4 HLS playlist in a real WKWebView and report decoding.
// Usage: wkcheck <http://127.0.0.1:port/dir/index.m3u8>
import Cocoa
import WebKit

final class Checker: NSObject, WKNavigationDelegate {
    let url: String
    let view: WKWebView
    var ticks = 0
    init(url: String) {
        self.url = url
        let config = WKWebViewConfiguration()
        config.mediaTypesRequiringUserActionForPlayback = []
        view = WKWebView(frame: NSRect(x: 0, y: 0, width: 640, height: 360), configuration: config)
        super.init()
    }
    func start() {
        // WebKit pauses media in hidden windows, so the window is on screen but nearly transparent.
        let window = NSWindow(contentRect: NSRect(x: 40, y: 40, width: 640, height: 360), styleMask: [.borderless], backing: .buffered, defer: false)
        window.contentView = view
        window.alphaValue = 0.02
        window.level = .floating
        window.orderFrontRegardless()
        // An inactive app's media can be throttled; make this window the active one.
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        let html = "<video id=v src=\"\(url)\" autoplay muted playsinline></video>"
        view.loadHTMLString(html, baseURL: URL(string: url))
        Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { timer in
            self.ticks += 1
            let js = "(()=>{const v=document.getElementById('v');if(v.paused&&v.readyState>=2)v.play().catch(()=>{});return JSON.stringify({t:v.currentTime,w:v.videoWidth,h:v.videoHeight,d:v.duration,err:v.error&&v.error.code,audio:v.webkitAudioDecodedByteCount,video:v.webkitVideoDecodedByteCount,ready:v.readyState,paused:v.paused})})()"
            self.view.evaluateJavaScript(js) { result, _ in
                guard let text = result as? String else { return }
                if let data = text.data(using: .utf8), let row = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                   ((row["t"] as? Double ?? 0) > 3 || row["err"] is NSNumber || self.ticks > 40) {
                    print("WKCHECK:\(text)")
                    timer.invalidate()
                    NSApp.terminate(nil)
                }
            }
        }
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let checker = Checker(url: CommandLine.arguments[1])
checker.start()
app.run()
