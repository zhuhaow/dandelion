//
//  Specht2App.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/11/11.
//

import AppKit

class AppDelegate: NSObject, NSApplicationDelegate {
    var taskBarController: TaskBarController!

    func applicationDidFinishLaunching(_ notification: Notification) {
        ConfigManager.initialize()
        Autostart.initialize()
        Update.initialize()

        taskBarController = TaskBarController()
    }

    func applicationWillTerminate(_ notification: Notification) {
        ConfigManager.blockShutdown()
    }
}
