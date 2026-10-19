# 1Password for the quickshell bar, through `op` and the desktop app integration.
# Secrets never pass through the bar: `list` and `fields` print metadata only (concealed and
# one-time-code values are stripped), and `copy` fetches a value straight onto the clipboard.
#
#   qs-op list                            items as JSON
#   qs-op fields <vault> <item>           an item's fields as JSON, without secret values
#   qs-op copy <vault> <item> <field|otp> [paste]

clear_after=30

case "${1:-}" in
    list)
        op item list --format json | jq -c '[.[] | {
            id, title, category,
            vault: .vault.id,
            vaultName: (.vault.name // ""),
            user: (.additional_information // ""),
            urls: ([.urls[]?.href] | join(" "))
        }]'
        ;;
    fields)
        op item get "$3" --vault "$2" --format json | jq -c '[.fields[]?
            | select(.type == "OTP" or ((.value // "") != ""))
            | {id, label, type, purpose, section: (.section.label // ""),
               value: (if .type == "CONCEALED" or .type == "OTP" then null else .value end)}]'
        ;;
    copy)
        if [ "${4:-}" = otp ]; then
            value=$(op item get "$3" --vault "$2" --otp)
        else
            value=$(op read --no-newline "op://$2/$3/$4")
        fi
        # Sensitive: cliphist doesn't record it.
        printf %s "$value" | wl-copy --sensitive
        # Clear it later, unless something else has been copied since. Only a hash of the
        # secret goes into the background job's arguments.
        hash=$(printf %s "$value" | sha256sum | cut -d' ' -f1)
        unset value
        # shellcheck disable=SC2016 # the inner script expands its own arguments
        setsid -f sh -c '
            sleep "$1"
            [ "$(wl-paste -n 2>/dev/null | sha256sum | cut -d" " -f1)" = "$2" ] && wl-copy --clear
        ' sh "$clear_after" "$hash"
        if [ "${5:-}" = paste ]; then
            qs-paste
        fi
        ;;
    *)
        echo "usage: qs-op list | fields <vault> <item> | copy <vault> <item> <field|otp> [paste]" >&2
        exit 2
        ;;
esac
