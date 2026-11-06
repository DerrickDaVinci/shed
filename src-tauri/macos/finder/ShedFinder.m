// The Finder extension: "Clean this Mac with Shed" when you right-click the
// Desktop or the empty space in any Finder window, and "Clean with Shed" on
// selected folders. It only opens Shed through its shed:// link; Shed does
// the work (the extension is sandboxed and touches no files).

#import <Cocoa/Cocoa.h>
#import <FinderSync/FinderSync.h>

@interface ShedFinder : FIFinderSync
@end

static BOOL zh(void) {
    NSString *lang = NSLocale.preferredLanguages.firstObject ?: @"";
    return [lang hasPrefix:@"zh"];
}

static void openShed(NSString *link) {
    [[NSWorkspace sharedWorkspace] openURL:[NSURL URLWithString:link]];
}

// The path as a query value: & = + ; and # must be escaped too.
static NSString *encoded(NSURL *url) {
    NSMutableCharacterSet *ok = NSCharacterSet.URLPathAllowedCharacterSet.mutableCopy;
    [ok removeCharactersInString:@"&=+;#"];
    return [url.path stringByAddingPercentEncodingWithAllowedCharacters:ok] ?: @"";
}

@implementation ShedFinder

- (instancetype)init {
    self = [super init];
    // Everywhere: the Desktop, home, external disks.
    FIFinderSyncController.defaultController.directoryURLs = [NSSet setWithObject:[NSURL fileURLWithPath:@"/"]];
    return self;
}

- (NSMenu *)menuForMenuKind:(FIMenuKind)kind {
    NSMenu *menu = [[NSMenu alloc] initWithTitle:@""];
    NSImage *icon = [NSImage imageWithSystemSymbolName:@"sparkles" accessibilityDescription:nil];
    if (kind == FIMenuKindContextualMenuForContainer) {
        NSMenuItem *all = [menu addItemWithTitle:(zh() ? @"用 Shed 清理这台电脑" : @"Clean This Mac with Shed") action:@selector(cleanMac:) keyEquivalent:@""];
        all.image = icon;
        NSURL *here = FIFinderSyncController.defaultController.targetedURL;
        NSURL *desktop = [NSFileManager.defaultManager URLsForDirectory:NSDesktopDirectory inDomains:NSUserDomainMask].firstObject;
        if (here && ![here.path isEqualToString:desktop.path]) {
            [menu addItemWithTitle:(zh() ? @"用 Shed 清理这个文件夹" : @"Clean This Folder with Shed") action:@selector(cleanHere:) keyEquivalent:@""].image = icon;
        }
    } else if (kind == FIMenuKindContextualMenuForItems) {
        [menu addItemWithTitle:(zh() ? @"用 Shed 清理" : @"Clean with Shed") action:@selector(cleanItems:) keyEquivalent:@""].image = icon;
    }
    return menu;
}

- (void)cleanMac:(id)sender {
    openShed(@"shed://computer");
}

- (void)cleanHere:(id)sender {
    NSURL *here = FIFinderSyncController.defaultController.targetedURL;
    if (here) openShed([@"shed://open?path=" stringByAppendingString:encoded(here)]);
}

- (void)cleanItems:(id)sender {
    NSMutableArray *parts = [NSMutableArray array];
    for (NSURL *u in FIFinderSyncController.defaultController.selectedItemURLs) {
        [parts addObject:[@"path=" stringByAppendingString:encoded(u)]];
    }
    if (parts.count) openShed([@"shed://open?" stringByAppendingString:[parts componentsJoinedByString:@"&"]]);
}

@end
