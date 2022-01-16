//
//  main.swift
//  me.zhuhaow.proxy-helper
//
//  Created by Zhuhao Wang on 2021/12/23.
//

import Foundation

specht2_init_syslog(UInt(Info.rawValue))

_ = Server()

RunLoop.main.run()
