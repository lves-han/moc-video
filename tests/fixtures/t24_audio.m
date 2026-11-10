#import <Cocoa/Cocoa.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <CoreMedia/CoreMedia.h>
#import <AudioToolbox/AudioToolbox.h>
#include <unistd.h>

// Test-only, application-filtered PCM observation. No screen output is installed.
@interface AudioSink : NSObject <SCStreamOutput, SCStreamDelegate>
@property FILE *pcm;
@property FILE *timing;
@property size_t blocks;
@property size_t frames;
@property double firstPTS;
@property double lastPTS;
@property BOOL failed;
@end
@implementation AudioSink
- (void)stream:(SCStream *)stream didStopWithError:(NSError *)error {
    fprintf(stderr, "Application audio stream failed: %s\n", error.localizedDescription.UTF8String);
    self.failed = YES;
}
- (void)stream:(SCStream *)stream didOutputSampleBuffer:(CMSampleBufferRef)sample ofType:(SCStreamOutputType)type {
    if (type != SCStreamOutputTypeAudio || !CMSampleBufferDataIsReady(sample)) return;
    const AudioStreamBasicDescription *format = CMAudioFormatDescriptionGetStreamBasicDescription(CMSampleBufferGetFormatDescription(sample));
    if (!format || format->mFormatID != kAudioFormatLinearPCM || !(format->mFormatFlags & kAudioFormatFlagIsFloat)
        || format->mBitsPerChannel != 32 || format->mChannelsPerFrame != 1 || format->mSampleRate != 48000) {
        fprintf(stderr, "Expected native float32 mono 48000Hz PCM\n"); self.failed = YES; return;
    }
    AudioBufferList list;
    CMBlockBufferRef block = NULL;
    OSStatus status = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(sample, NULL, &list, sizeof(list),
        NULL, NULL, kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment, &block);
    if (status || list.mNumberBuffers != 1 || !list.mBuffers[0].mData) { self.failed = YES; if (block) CFRelease(block); return; }
    AudioBuffer buffer = list.mBuffers[0];
    size_t frames = buffer.mDataByteSize / sizeof(float);
    double pts = CMTimeGetSeconds(CMSampleBufferGetPresentationTimeStamp(sample));
    if (frames != CMSampleBufferGetNumSamples(sample) || !isfinite(pts)) self.failed = YES;
    if (fprintf(self.timing, "{\"index\":%zu,\"offset_frames\":%zu,\"frames\":%zu,\"pts\":%.9f}\n",
        self.blocks++, self.frames, frames, pts) < 0 || fflush(self.timing)) self.failed = YES;
    if (fwrite(buffer.mData, 1, buffer.mDataByteSize, self.pcm) != buffer.mDataByteSize) self.failed = YES;
    if (fflush(self.pcm)) self.failed = YES;
    if (!self.frames) self.firstPTS = pts;
    self.lastPTS = pts + (double)CMSampleBufferGetNumSamples(sample) / 48000;
    self.frames += buffer.mDataByteSize / sizeof(float);
    CFRelease(block);
}
@end

static void emit(NSDictionary *value) {
    NSData *data = [NSJSONSerialization dataWithJSONObject:value options:0 error:nil];
    puts([[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding].UTF8String); fflush(stdout);
}
int main(int argc, const char **argv) {
    @autoreleasepool {
        if (argc != 4) return 2;
        if (!CGPreflightScreenCaptureAccess()) { fprintf(stderr, "Screen recording consent required for owned-application audio capture\n"); return 3; }
        pid_t pid = atoi(argv[1]);
        AudioSink *sink = [AudioSink new];
        sink.pcm = fopen(argv[2], "wbx");
        sink.timing = fopen(argv[3], "wbx");
        if (!sink.pcm || !sink.timing) return 4;
        __block SCStream *stream;
        dispatch_queue_t queue = dispatch_queue_create("t24-owned-audio", DISPATCH_QUEUE_SERIAL);
        [SCShareableContent getShareableContentExcludingDesktopWindows:YES onScreenWindowsOnly:YES completionHandler:^(SCShareableContent *content, NSError *error) {
            SCRunningApplication *owned = nil;
            for (SCRunningApplication *app in content.applications) if (app.processID == pid) owned = app;
            if (error || !owned || !content.displays.count) { fprintf(stderr, "Owned audio application/display unavailable\n"); exit(5); }
            SCContentFilter *filter = [[SCContentFilter alloc] initWithDisplay:content.displays[0] includingApplications:@[owned] exceptingWindows:@[]];
            SCStreamConfiguration *config = [SCStreamConfiguration new];
            config.width = 2; config.height = 2; config.minimumFrameInterval = CMTimeMake(1, 1);
            config.capturesAudio = YES; config.sampleRate = 48000; config.channelCount = 1; config.excludesCurrentProcessAudio = YES;
            stream = [[SCStream alloc] initWithFilter:filter configuration:config delegate:sink];
            NSError *failure = nil;
            if (![stream addStreamOutput:sink type:SCStreamOutputTypeAudio sampleHandlerQueue:queue error:&failure]) exit(6);
            [stream startCaptureWithCompletionHandler:^(NSError *failure) {
                if (failure) { fprintf(stderr, "Owned audio capture start failed: %s\n", failure.localizedDescription.UTF8String); exit(7); }
                emit(@{@"started": @YES, @"pid": @(pid), @"sample_rate": @48000, @"channels": @1, @"format": @"f32le", @"scope": @"owned application only"});
                dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
                    char command; ssize_t count = read(STDIN_FILENO, &command, 1);
                    [stream stopCaptureWithCompletionHandler:^(NSError *failure) {
                        dispatch_async(queue, ^{
                            int status = fclose(sink.pcm) | fclose(sink.timing);
                            emit(@{@"stopped": @YES, @"frames": @(sink.frames), @"first_pts": @(sink.firstPTS), @"last_pts": @(sink.lastPTS),
                                @"seconds": @((double)sink.frames / 48000), @"failed": @(sink.failed || failure || status || count != 1)});
                            exit(sink.failed || failure || status || count != 1 ? 8 : 0);
                        });
                    }];
                });
            }];
        }];
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 720 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{ fprintf(stderr, "Owned capture exceeded bounded lifetime\n"); exit(9); });
        [[NSRunLoop currentRunLoop] run];
    }
}
