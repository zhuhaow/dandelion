//
//  Constants.swift
//  me.zhuhaow.Specht2.proxy-helper
//
//  Created by Zhuhao Wang on 2021/12/23.
//

import Foundation

class Constants {
    static let helperMachLabel = "me.zhuhaow.Specht2.proxy-helper"

    // Bump this so the main app will update the XPC service
    static let version = "35"

    static let serviceCodeSignRequirements = "identifier \"\(Constants.helperMachLabel)\"" +
        " and anchor apple generic and certificate leaf[subject.OU] = \"H5443445N6\""

    static let clientCodeSignRequirements = "identifier \"me.zhuhaow.Specht2\"" +
        " and anchor apple generic and certificate leaf[subject.OU] = \"H5443445N6\""
}
