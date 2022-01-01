#!/bin/sh

#  uninstall.sh
#  Specht2
#
#  Created by Zhuhao Wang on 2021/12/31.
#  

sudo launchctl unload /Library/LaunchDaemons/me.zhuhaow.Specht2.proxy-helper.plist
sudo rm /Library/LaunchDaemons/me.zhuhaow.Specht2.proxy-helper.plist
sudo rm /Library/PrivilegedHelperTools/me.zhuhaow.Specht2.proxy-helper
