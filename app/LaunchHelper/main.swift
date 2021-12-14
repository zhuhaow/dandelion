//
//  main.swift
//  Specht2LaunchHelper
//
//  Created by Zhuhao Wang on 2021/12/14.
//

import AppKit

let mainAppBundleID = "me.zhuhaow.Specht2"

let runningApps = NSWorkspace.shared.runningApplications
let isRunning = runningApps.contains {
    $0.bundleIdentifier == mainAppBundleID
}

if !isRunning {
    var path = Bundle.main.bundlePath as NSString
    for _ in 1...4 {
        path = path.deletingLastPathComponent as NSString
    }
    let applicationPathString = path as String
    guard let pathURL = URL(string: applicationPathString) else { exit(1) }
    NSWorkspace.shared.openApplication(at: pathURL,
                                       configuration: NSWorkspace.OpenConfiguration(),
                                       completionHandler: nil)
}
