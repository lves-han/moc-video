#import <Foundation/Foundation.h>
#import <WebKit/WebKit.h>
#include <stdio.h>

// T00 checks the installed SDK and dynamic framework; T01 exercises a real window.
int main(void) {
    @autoreleasepool {
        if (NSClassFromString(@"WKWebView") != [WKWebView class]) {
            return 1;
        }
        puts("T00 WebKit link: WKWebView available");
    }
    return 0;
}
