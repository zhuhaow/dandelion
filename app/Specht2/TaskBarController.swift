//
//  TaskBarController.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/12/15.
//

import Foundation
import AppKit

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

        menu.addItem(withTitle: "Open config folder", action: #selector(self.openConfigFolder), keyEquivalent: "").target = self
    }

    @objc func openConfigFolder() {
        ConfigManager.openConfigFolder()
    }
}
