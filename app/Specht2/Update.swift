//
//  Update.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/12/17.
//

import Foundation
import Sparkle
import Defaults

class Update {
    static func initialize() {
        shared.autoUpdate(enable: Defaults[.autoUpdate])
        shared.startUpdater()
    }

    static var shared: Update = {
        Update()
    }()

    static func checkUpdate() {
        shared.checkUpdate()
    }

    // swiftlint:disable weak_delegate
    private let delegate: UpdateDelegate
    private let controller: SPUStandardUpdaterController

    private init() {
        delegate = UpdateDelegate()
        controller = SPUStandardUpdaterController(startingUpdater: false,
                                                  updaterDelegate: delegate,
                                                  userDriverDelegate: nil)
    }

    func checkUpdate() {
        controller.checkForUpdates(nil)
    }

    func autoUpdate(enable: Bool) {
        Defaults[.autoUpdate] = enable
        controller.updater.automaticallyChecksForUpdates = enable
    }

    private func startUpdater() {
        controller.startUpdater()
    }
}

private class UpdateDelegate: NSObject, SPUUpdaterDelegate {
    func allowedChannels(for updater: SPUUpdater) -> Set<String> {
        if Defaults[.useBetaChannel] {
            return ["beta"]
        } else {
            return []
        }
    }
}
