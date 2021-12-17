//
//  Defaults.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/12/14.
//

import Foundation
import Defaults

extension Defaults.Keys {
    static let activeConfig = Key<String?>("activeConfig")
    static let setSystemProxy = Key<Bool>("setSystemProxy", default: true)
    static let localhostOnly = Key<Bool>("localhostOnly", default: true)
    static let autostart = Key<Bool>("autostart", default: true)
    static let autoUpdate = Key<Bool>("autoUpdate", default: true)
    static let useBetaChannel = Key<Bool>("useBetaChannel", default: false)
}
