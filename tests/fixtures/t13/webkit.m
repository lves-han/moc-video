#import <Cocoa/Cocoa.h>
#import <WebKit/WebKit.h>
#import <objc/message.h>
#include <unistd.h>

// Test-only native window. All scripts, screenshots and process diagnostics stay under tests/evidence.
static int resultCode = 1;
@interface Browser : NSObject <WKNavigationDelegate, WKScriptMessageHandlerWithReply>
@property WKWebView *view;
@property NSWindow *window;
@property NSString *script;
@property NSString *report;
@property NSString *folder;
@property NSString *startupScript;
@property NSDictionary *beforeReload;
@property BOOL launched;
@property BOOL finished;
@end

@implementation Browser
- (void)write:(id)value path:(NSString *)path {
    NSError *error = nil;
    NSData *data = [NSJSONSerialization dataWithJSONObject:value options:NSJSONWritingPrettyPrinted error:&error];
    if (!data || ![data writeToFile:path options:NSDataWritingAtomic error:&error]) {
        fprintf(stderr, "T13 report write: %s\n", error.description.UTF8String);
        resultCode = 1;
    }
}
- (void)finish:(NSDictionary *)value {
    if (self.finished) return;
    self.finished = YES;
    if (self.beforeReload) {
        NSMutableDictionary *merged = [self.beforeReload mutableCopy];
        merged[@"ok"] = [self.beforeReload[@"ok"] boolValue] && [value[@"ok"] boolValue] ? @YES : @NO;
        merged[@"cases"] = [self.beforeReload[@"cases"] arrayByAddingObjectsFromArray:value[@"cases"] ?: @[]];
        merged[@"screenshots"] = [self.beforeReload[@"screenshots"] arrayByAddingObjectsFromArray:value[@"screenshots"] ?: @[]];
        if (value[@"error"]) merged[@"reload_error"] = value[@"error"];
        value = merged;
    }
    resultCode = [value[@"ok"] boolValue] ? 0 : 1;
    [self write:value path:self.report];
    [self.view stopLoading];
    [self.view.configuration.userContentController removeScriptMessageHandlerForName:@"t13" contentWorld:WKContentWorld.pageWorld];
    [self.window close];
    [NSApp stop:nil];
    // stop: changes the run flag; wake nextEventMatchingMask: so run can return.
    [NSApp postEvent:[NSEvent otherEventWithType:NSEventTypeApplicationDefined
        location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0
        context:nil subtype:0 data1:0 data2:0] atStart:YES];
}
- (NSDictionary *)identity {
    NSMutableDictionary *pids = [NSMutableDictionary dictionary];
    for (NSArray *row in @[@[@"renderer", self.view, @"_webProcessIdentifier"],
                           @[@"gpu", self.view, @"_gpuProcessIdentifier"],
                           @[@"network", self.view.configuration.websiteDataStore, @"_networkProcessIdentifier"]]) {
        SEL selector = NSSelectorFromString(row[2]);
        if (![row[1] respondsToSelector:selector]) {
            return @{@"pid": @(getpid()), @"error": [@"Missing WebKit diagnostic " stringByAppendingString:row[2]]};
        }
        pid_t pid = ((pid_t (*)(id, SEL))objc_msgSend)(row[1], selector);
        pids[row[0]] = @(pid);
    }
    return @{@"pid": @(getpid()), @"webkit_pids": pids, @"width": @(self.view.bounds.size.width),
             @"height": @(self.view.bounds.size.height), @"zoom": @(self.view.pageZoom),
             @"backing_scale": @(self.window.backingScaleFactor)};
}
- (void)sample {
    if (!self.finished) [self write:[self identity] path:[self.folder stringByAppendingPathComponent:@"native-status.json"]];
}
- (void)webView:(WKWebView *)webView didFinishNavigation:(WKNavigation *)navigation {
    if (self.launched) return;
    self.launched = YES;
    [self sample];
    [webView evaluateJavaScript:self.script completionHandler:^(id value, NSError *error) {
        if (error) [self finish:@{@"ok": @NO, @"error": error.description, @"cases": @[]}];
    }];
}
- (void)webView:(WKWebView *)webView didFailNavigation:(WKNavigation *)navigation withError:(NSError *)error {
    [self finish:@{@"ok": @NO, @"error": error.description, @"cases": @[]}];
}
- (void)webView:(WKWebView *)webView didFailProvisionalNavigation:(WKNavigation *)navigation withError:(NSError *)error {
    [self webView:webView didFailNavigation:navigation withError:error];
}
- (void)userContentController:(WKUserContentController *)controller didReceiveScriptMessage:(WKScriptMessage *)message
                 replyHandler:(void (^)(id, NSString *))reply {
    NSDictionary *body = message.body;
    NSString *action = body[@"action"];
    if ([action isEqual:@"resize"]) {
        [self.window setContentSize:NSMakeSize([body[@"width"] doubleValue], [body[@"height"] doubleValue])];
        self.view.pageZoom = [body[@"zoom"] doubleValue] ?: 1;
        [self sample];
        reply([self identity], nil);
    } else if ([action isEqual:@"appearance"]) {
        NSAppearance *appearance = [NSAppearance appearanceNamed:[body[@"dark"] boolValue] ? NSAppearanceNameDarkAqua : NSAppearanceNameAqua];
        NSApp.appearance = appearance; self.window.appearance = appearance; self.view.appearance = appearance;
        reply(@YES, nil);
    } else if ([action isEqual:@"screenshot"]) {
        NSString *name = body[@"name"];
        if (![name.lastPathComponent isEqual:name] || ![name.pathExtension isEqual:@"png"]) { reply(nil, @"Invalid screenshot name"); return; }
        [self.view takeSnapshotWithConfiguration:nil completionHandler:^(NSImage *image, NSError *error) {
            if (error || !image) { reply(nil, error.description ?: @"No snapshot"); return; }
            NSBitmapImageRep *bitmap = [NSBitmapImageRep imageRepWithData:image.TIFFRepresentation];
            NSData *png = [bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
            NSString *path = [self.folder stringByAppendingPathComponent:name];
            NSError *writeError = nil;
            if (!png || ![png writeToFile:path options:NSDataWritingAtomic error:&writeError]) { reply(nil, writeError.description ?: @"Empty PNG"); return; }
            reply(@{@"file": name, @"width": @(bitmap.pixelsWide), @"height": @(bitmap.pixelsHigh), @"bytes": @(png.length)}, nil);
        }];
    } else if ([action isEqual:@"identity"]) {
        [self sample]; reply([self identity], nil);
    } else if ([action isEqual:@"reload-history"]) {
        self.beforeReload = body[@"report"];
        NSError *error = nil;
        self.script = [NSString stringWithContentsOfFile:self.startupScript encoding:NSUTF8StringEncoding error:&error];
        if (!self.script) { reply(nil, error.description); [self finish:@{@"ok": @NO, @"cases": @[], @"error": error.description}]; return; }
        self.launched = NO;
        reply(@YES, nil);
        dispatch_async(dispatch_get_main_queue(), ^{ [self.view reload]; });
    } else if ([action isEqual:@"finish"]) {
        reply(@YES, nil);
        dispatch_async(dispatch_get_main_queue(), ^{ [self finish:body[@"report"]]; });
    } else reply(nil, @"Unknown T13 action");
}
@end

int main(int argc, const char **argv) {
    @autoreleasepool {
        if (argc != 5 && argc != 6) { fprintf(stderr, "url script report folder [seed script] required\n"); return 2; }
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
        Browser *browser = [Browser new];
        NSError *error = nil;
        browser.script = [NSString stringWithContentsOfFile:@(argv[2]) encoding:NSUTF8StringEncoding error:&error];
        if (!browser.script) { fprintf(stderr, "%s\n", error.description.UTF8String); return 2; }
        browser.report = @(argv[3]); browser.folder = @(argv[4]);
        browser.startupScript = [[@(argv[2]) stringByDeletingLastPathComponent] stringByAppendingPathComponent:@"startup.js"];
        WKWebViewConfiguration *config = [WKWebViewConfiguration new];
        config.websiteDataStore = [WKWebsiteDataStore nonPersistentDataStore];
        config.mediaTypesRequiringUserActionForPlayback = WKAudiovisualMediaTypeNone;
        [config.userContentController addScriptMessageHandlerWithReply:browser contentWorld:WKContentWorld.pageWorld name:@"t13"];
        NSString *diagnostics = @"window.__T13_LOGS__=[];for(const level of ['error','warn']){const original=console[level];console[level]=(...args)=>{window.__T13_LOGS__.push({level,text:args.map(String).join(' ')});original.apply(console,args)}};addEventListener('error',e=>{if(e.message)window.__T13_LOGS__.push({level:'error',text:e.message})});addEventListener('unhandledrejection',e=>window.__T13_LOGS__.push({level:'error',text:String(e.reason)}));";
        [config.userContentController addUserScript:[[WKUserScript alloc] initWithSource:diagnostics injectionTime:WKUserScriptInjectionTimeAtDocumentStart forMainFrameOnly:YES]];
        NSString *firstFrame = @"new MutationObserver((records,observer)=>{if(document.querySelector('.app-shell')){window.__T13_FIRST_FRAME__={palette:document.documentElement.dataset.palette,lang:document.documentElement.lang,accent:getComputedStyle(document.documentElement).getPropertyValue('--accent').trim(),scheme:document.documentElement.style.colorScheme,dark:matchMedia('(prefers-color-scheme: dark)').matches,at:performance.now()};observer.disconnect()}}).observe(document,{childList:true,subtree:true});";
        [config.userContentController addUserScript:[[WKUserScript alloc] initWithSource:firstFrame injectionTime:WKUserScriptInjectionTimeAtDocumentStart forMainFrameOnly:YES]];
        if (argc == 6) {
            NSString *seed = [NSString stringWithContentsOfFile:@(argv[5]) encoding:NSUTF8StringEncoding error:&error];
            if (!seed) { fprintf(stderr, "%s\n", error.description.UTF8String); return 2; }
            [config.userContentController addUserScript:[[WKUserScript alloc] initWithSource:seed injectionTime:WKUserScriptInjectionTimeAtDocumentStart forMainFrameOnly:YES]];
        }
        browser.view = [[WKWebView alloc] initWithFrame:NSMakeRect(0, 0, 1200, 800) configuration:config];
        browser.view.navigationDelegate = browser;
        browser.view.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        browser.window = [[NSWindow alloc] initWithContentRect:browser.view.frame styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskResizable backing:NSBackingStoreBuffered defer:NO];
        browser.window.releasedWhenClosed = NO;
        browser.window.title = @"T13 UI acceptance";
        browser.window.contentView = browser.view;
        [browser.window center]; [browser.window makeKeyAndOrderFront:nil]; [NSApp activateIgnoringOtherApps:YES];
        [browser.view loadRequest:[NSURLRequest requestWithURL:[NSURL URLWithString:@(argv[1])]]];
        [NSTimer scheduledTimerWithTimeInterval:0.5 repeats:YES block:^(NSTimer *timer) { [browser sample]; }];
        [NSTimer scheduledTimerWithTimeInterval:900 repeats:NO block:^(NSTimer *timer) {
            [browser finish:@{@"ok": @NO, @"error": @"UI acceptance exceeded 900 seconds", @"cases": @[]}];
        }];
        [NSApp run];
        fprintf(stderr, "T13 AppKit run returned; exit=%d\n", resultCode);
    }
    return resultCode;
}
