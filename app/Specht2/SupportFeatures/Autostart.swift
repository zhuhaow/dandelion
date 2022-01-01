//
//  Autostart.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/12/16.
//

import Foundation
import ServiceManagement
import Defaults

class Autostart {
    static let identifier = "me.zhuhaow.Specht2.Specht2LaunchHelper"

    static func enable() {
        setState(true)
    }

    static func disable() {
        setState(false)
    }

    static func setState(_ enabled: Bool) {
        Defaults[.autostart] = enabled
        SMLoginItemSetEnabled(identifier as CFString, enabled)
    }

    static func refreshState() {
        SMLoginItemSetEnabled(identifier as CFString, Defaults[.autostart])
    }

    static func state() -> Bool {
        return Defaults[.autostart]
    }

    static func toggle() {
        setState(!state())
    }

    static func initialize() {
        refreshState()
    }
}

extension Defaults.Keys {
    static let autostart = Key<Bool>("autostart", default: true)
}
