#import <Cocoa/Cocoa.h>

// Passive diagnostics, loaded only into the owned acceptance app. No responder,
// event, window or system setting is changed by this observer.
static void record(NSString *stage, NSEvent *event) {
    NSWindow *window = NSApp.keyWindow;
    NSMutableArray *chain = [NSMutableArray array];
    NSResponder *responder = window.firstResponder;
    for (int i = 0; responder && i < 8; i++, responder = responder.nextResponder)
        [chain addObject:NSStringFromClass(responder.class)];
    NSMutableDictionary *row = [@{@"stage":stage, @"at":@(NSProcessInfo.processInfo.systemUptime),
        @"key_window":@(window.isKeyWindow), @"main_window":@(window.isMainWindow),
        @"responder_chain":chain, @"fullscreen":@((window.styleMask & NSWindowStyleMaskFullScreen) != 0)} mutableCopy];
    if (event) {
        row[@"key_code"] = @(event.keyCode);
        row[@"event_type"] = @(event.type);
    }
    NSData *data = [NSJSONSerialization dataWithJSONObject:row options:0 error:nil];
    fprintf(stderr, "TV6_APPKIT:%s\n", [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding].UTF8String);
    fflush(stderr);
}

__attribute__((constructor)) static void observe(void) {
    if (![NSProcessInfo.processInfo.processName isEqual:@"moc-video"]) return;
    dispatch_async(dispatch_get_main_queue(), ^{
        for (NSString *name in @[NSWindowWillEnterFullScreenNotification, NSWindowDidEnterFullScreenNotification,
                NSWindowWillExitFullScreenNotification, NSWindowDidExitFullScreenNotification,
                NSWindowDidBecomeKeyNotification, NSWindowDidResignKeyNotification]) {
            [NSNotificationCenter.defaultCenter addObserverForName:name object:nil queue:nil
                usingBlock:^(NSNotification *notification) { record(notification.name, nil); }];
        }
        [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskKeyDown | NSEventMaskKeyUp handler:^NSEvent *(NSEvent *event) {
            if (event.keyCode == 53 || event.keyCode == 46) {
                record(@"key-before", event);
                dispatch_async(dispatch_get_main_queue(), ^{ record(@"key-after", event); });
            }
            return event;
        }];
        record(@"observer-ready", nil);
    });
}
