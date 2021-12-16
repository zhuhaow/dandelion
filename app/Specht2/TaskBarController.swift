//
//  TaskBarController.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/12/15.
//

import Foundation
import AppKit
import Defaults
import Then

class TaskBarController: NSObject, NSMenuDelegate {
    let statusItem: NSStatusItem

    override init() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)

        statusItem.button?.title = "S"
        super.init()

        statusItem.menu = NSMenu()
        statusItem.menu!.delegate = self
    }

    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()

        ConfigManager.configs.forEach {
            menu.addItem(itemForConfig(name: $0.key, path: $0.value))
        }

        menu.addItem(NSMenuItem.separator())

        menu.addItem(withTitle: "Stop server",
                     action: #selector(self.stopServer),
                     keyEquivalent: "")
            .target = self

        menu.addItem(withTitle: "Reload config",
                     action: #selector(self.reloadConfig),
                     keyEquivalent: "")
            .target = self

        menu.addItem(withTitle: "Open config folder",
                     action: #selector(self.openConfigFolder),
                     keyEquivalent: "")
            .target = self

        menu.addItem(NSMenuItem.separator())

        _ = menu.addItem(withTitle: "Autostart app at login",
                     action: #selector(self.setAutostart),
                     keyEquivalent: "").then {
            $0.target = self
            if Defaults[.autostart] {
                $0.state = .on
            }
        }
        menu.addItem(withTitle: "About",
                     action: #selector(self.about),
                     keyEquivalent: "")
            .target = self

        menu.addItem(NSMenuItem.separator())

        menu.addItem(withTitle: "Exit",
                     action: #selector(self.exit),
                     keyEquivalent: "")
            .target = self
    }

}

extension TaskBarController {
    func itemForConfig(name: String, path: URL) -> NSMenuItem {
        let item = NSMenuItem(title: name, action: #selector(self.startServer), keyEquivalent: "")

        item.target = self

        if name == ConfigManager.activeConfig {
            item.state = .on
        }

        return item
    }

    @objc func startServer(sender: NSMenuItem) {
        ConfigManager.run(name: sender.title)
    }

    @objc func stopServer() {
        ConfigManager.stop()
    }

    @objc func openConfigFolder() {
        ConfigManager.openConfigFolder()
    }

    @objc func reloadConfig() {
        ConfigManager.reloadConfigs()
    }
}

extension TaskBarController {
    @objc func setAutostart() {
        Defaults[.autostart].toggle()
        Autostart.setState(Defaults[.autostart])
    }

    @objc func about() {
        NSApplication.shared.activate(ignoringOtherApps: true)
        NSApplication.shared.orderFrontStandardAboutPanel(nil)
    }
}

extension TaskBarController {
    @objc func exit() {
        NSApp.terminate(nil)
    }
}
