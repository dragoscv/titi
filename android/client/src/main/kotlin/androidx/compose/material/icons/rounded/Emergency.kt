/*
 * Copyright 2025 The Android Open Source Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package androidx.compose.material.icons.rounded

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.materialIcon
import androidx.compose.material.icons.materialPath
import androidx.compose.ui.graphics.vector.ImageVector

public val Icons.Rounded.Emergency: ImageVector
    get() {
        if (_emergency != null) {
            return _emergency!!
        }
        _emergency = materialIcon(name = "Rounded.Emergency") {
            materialPath {
                moveTo(20.29f, 8.37f)
                lineToRelative(-1.0f, -1.73f)
                curveToRelative(-0.28f, -0.48f, -0.89f, -0.64f, -1.37f, -0.37f)
                lineTo(14.0f, 8.54f)
                verticalLineTo(4.0f)
                curveToRelative(0.0f, -0.55f, -0.45f, -1.0f, -1.0f, -1.0f)
                horizontalLineToRelative(-2.0f)
                curveToRelative(-0.55f, 0.0f, -1.0f, 0.45f, -1.0f, 1.0f)
                verticalLineToRelative(4.54f)
                lineTo(6.07f, 6.27f)
                curveTo(5.59f, 5.99f, 4.98f, 6.16f, 4.71f, 6.63f)
                lineToRelative(-1.0f, 1.73f)
                curveTo(3.43f, 8.84f, 3.59f, 9.46f, 4.07f, 9.73f)
                lineTo(8.0f, 12.0f)
                lineToRelative(-3.93f, 2.27f)
                curveToRelative(-0.48f, 0.28f, -0.64f, 0.89f, -0.37f, 1.37f)
                lineToRelative(1.0f, 1.73f)
                curveToRelative(0.28f, 0.48f, 0.89f, 0.64f, 1.37f, 0.37f)
                lineTo(10.0f, 15.46f)
                verticalLineTo(20.0f)
                curveToRelative(0.0f, 0.55f, 0.45f, 1.0f, 1.0f, 1.0f)
                horizontalLineToRelative(2.0f)
                curveToRelative(0.55f, 0.0f, 1.0f, -0.45f, 1.0f, -1.0f)
                verticalLineToRelative(-4.54f)
                lineToRelative(3.93f, 2.27f)
                curveToRelative(0.48f, 0.28f, 1.09f, 0.11f, 1.37f, -0.37f)
                lineToRelative(1.0f, -1.73f)
                curveToRelative(0.28f, -0.48f, 0.11f, -1.09f, -0.37f, -1.37f)
                lineTo(16.0f, 12.0f)
                lineToRelative(3.93f, -2.27f)
                curveTo(20.41f, 9.46f, 20.57f, 8.84f, 20.29f, 8.37f)
                close()
            }
        }
        return _emergency!!
    }

private var _emergency: ImageVector? = null
