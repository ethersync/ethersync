# SPDX-FileCopyrightText: 2024 MangoIV <contact@mangoiv.com>
#
# SPDX-License-Identifier: AGPL-3.0-or-later

{vimUtils, ...}:
vimUtils.buildVimPlugin {
  name = "ethersync";
  src = ../../vim-plugin;
}
