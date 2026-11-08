using System.Runtime.InteropServices;
using System.Windows.Input;
using Windows.Win32;
using Windows.Win32.UI.Input.KeyboardAndMouse;

namespace OmniSpot.Input;

internal static class KeyboardInput
{
    public static bool SendSyntheticKeyDownUp(Key key)
    {
        var virtualKey = (VIRTUAL_KEY)KeyInterop.VirtualKeyFromKey(key);

        KEYBD_EVENT_FLAGS downFlags = 0, upFlags = KEYBD_EVENT_FLAGS.KEYEVENTF_KEYUP;

        if (IsExtendedKey(key))
        {
            downFlags = KEYBD_EVENT_FLAGS.KEYEVENTF_EXTENDEDKEY;
            upFlags |= KEYBD_EVENT_FLAGS.KEYEVENTF_EXTENDEDKEY;
        }

        INPUT[] inputs =
        [
            KeyEventInput(virtualKey, downFlags),
            KeyEventInput(virtualKey, upFlags)
        ];

        var result = PInvoke.SendInput(inputs, Marshal.SizeOf(typeof(INPUT)));

        return result == inputs.Length;

        INPUT KeyEventInput(VIRTUAL_KEY vKey, KEYBD_EVENT_FLAGS flags)
        {
            return new()
            {
                type = INPUT_TYPE.INPUT_KEYBOARD,
                Anonymous = new()
                {
                    ki = new KEYBDINPUT
                    {
                        wVk = vKey,
                        wScan = 0,
                        dwFlags = flags,
                        time = 0,
                        dwExtraInfo = 0
                    }
                }
            };
        }
    }

    /// <summary>
    /// Atomically sends modifier-down, key-down, key-up, modifier-up as a single <see cref="PInvoke.SendInput"/> call.
    /// Use this to replay a suppressed modifier+key combination so Windows can act on it.
    /// </summary>
    public static bool SendSyntheticCombination(Key modifier, Key actionKey)
    {
        var modVk = (VIRTUAL_KEY)KeyInterop.VirtualKeyFromKey(modifier);
        var actionVk = (VIRTUAL_KEY)KeyInterop.VirtualKeyFromKey(actionKey);

        var modDown = IsExtendedKey(modifier) ? KEYBD_EVENT_FLAGS.KEYEVENTF_EXTENDEDKEY : 0;
        var modUp = modDown | KEYBD_EVENT_FLAGS.KEYEVENTF_KEYUP;
        var actionDown = IsExtendedKey(actionKey) ? KEYBD_EVENT_FLAGS.KEYEVENTF_EXTENDEDKEY : 0;
        var actionUp = actionDown | KEYBD_EVENT_FLAGS.KEYEVENTF_KEYUP;

        INPUT[] inputs =
        [
            KeyEventInput(modVk, modDown),
            KeyEventInput(actionVk, actionDown),
            KeyEventInput(actionVk, actionUp),
            KeyEventInput(modVk, modUp),
        ];

        return PInvoke.SendInput(inputs, Marshal.SizeOf(typeof(INPUT))) == inputs.Length;

        static INPUT KeyEventInput(VIRTUAL_KEY vKey, KEYBD_EVENT_FLAGS flags) => new()
        {
            type = INPUT_TYPE.INPUT_KEYBOARD,
            Anonymous = new()
            {
                ki = new KEYBDINPUT
                {
                    wVk = vKey,
                    wScan = 0,
                    dwFlags = flags,
                    time = 0,
                    dwExtraInfo = 0
                }
            }
        };
    }

    private static bool IsExtendedKey(Key key) => key is Key.RightCtrl or Key.RightAlt or Key.Apps;

    /// <summary>
    /// Checks whether the given key is physically held down at the hardware/OS level.
    /// This prevents stuck state when Windows drops a KeyUp event (e.g. during elevation or desktop transitions).
    /// </summary>
    public static bool IsKeyPhysicallyDown(Key key)
    {
        if (key == Key.System)
        {
            return (PInvoke.GetAsyncKeyState((int)VIRTUAL_KEY.VK_MENU) & 0x8000) != 0;
        }

        var vk = KeyInterop.VirtualKeyFromKey(key);
        if (vk == 0)
        {
            return false;
        }

        if ((PInvoke.GetAsyncKeyState(vk) & 0x8000) != 0)
        {
            return true;
        }

        if (key is Key.LeftAlt or Key.RightAlt)
        {
            return (PInvoke.GetAsyncKeyState((int)VIRTUAL_KEY.VK_MENU) & 0x8000) != 0;
        }

        if (key is Key.LeftCtrl or Key.RightCtrl)
        {
            return (PInvoke.GetAsyncKeyState((int)VIRTUAL_KEY.VK_CONTROL) & 0x8000) != 0;
        }

        if (key is Key.LeftShift or Key.RightShift)
        {
            return (PInvoke.GetAsyncKeyState((int)VIRTUAL_KEY.VK_SHIFT) & 0x8000) != 0;
        }

        if (key is Key.LWin or Key.RWin)
        {
            return (PInvoke.GetAsyncKeyState((int)VIRTUAL_KEY.VK_LWIN) & 0x8000) != 0 ||
                   (PInvoke.GetAsyncKeyState((int)VIRTUAL_KEY.VK_RWIN) & 0x8000) != 0;
        }

        return false;
    }
}