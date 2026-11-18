set +avx; unset cfg p l part sum fixed thin;
sid=$0; owned=; t=$(stty -g) || exit 125;
m() { command printf "\036%s:%s\037" "$sid" "$1"; };
finish() { rc=$?; [ "$owned" ] || m "PREP:$rc"; stty "$t" && m RESTORED; };
trap finish 0; trap "exit 130" 2 15;
stty icanon isig intr ^C icrnl -inlcr -igncr -echo -noflsh || exit 125;
: UNFOUR_CURL_V4;
@SETUP@
r() { IFS= read -r a; }; bad() { m BAD; while r; do :; done; exit 125; };
m LOAD; r || exit; m PULSE; IFS= read -r fixed || exit;
if [ "${#fixed}" != @LENGTH@ ]; then thin=1; fixed=; m SHORT; while [ "${#fixed}" -lt @LENGTH@ ]; do r || exit; case "$a" in "") continue;; esac; fixed=$fixed$a; m L:${#fixed}; done; fi;
if sum=$(command printf %s "$fixed" | command sha256sum 2>/dev/null); then case "$sum" in @HASH@" "*) ;; *) bad;; esac;
else m CHECK; command printf %s "$fixed"; m VERIFY; while r; do case "$a" in OK:$sid) break;; "") ;; *) bad;; esac; done; fi; eval "$fixed"
@LINE@
fmt=octal; x=$(printf eA== | command base64 -d 2>/dev/null) && [ "$x" = x ] && fmt=b64;
ctx=-; @CONTEXT@
rd() { while IFS= read -r l; do case "$l" in "") ;; *) return 0;; esac; done; return 1; };
park() { while rd; do case "$l" in "Q:$sid") return 0;; esac; done; };
(@OPTIONS@ command -v base64 >/dev/null || exit 123) || { m "FAIL:$?"; park; exit 125; };
invalid() { m INPUT_INVALID; park; exit 125; };
lane=D; @SELECT_LANE@ case "$thin" in 1) lane=D;; esac; owned=1; m "READY:4:$fmt:$ctx:$lane";
p=; n=0; while :; do rd || exit 125; case "$l" in
"$lane"*) part=${l#?};
p=$p$part; n=$((n+1)); [ "$lane" = B ] || m "INPUT:$n:${#part}:$part";;
E:*) case "$lane:$l" in "D:E:$n:${#p}") ;; *) invalid;; esac; break;;
F:*) [ "$lane:$fmt" = B:b64 ] && sum=$(printf %s "$p" | command sha256sum 2>/dev/null) || invalid; case "$l" in "F:$n:${#p}:${sum%% *}") ;; *) invalid;; esac; break;;
*) invalid;; esac; done;
case $fmt in b64) cfg=$(printf %s "$p" | command base64 -d) || invalid;; *) cfg=$(printf %b "$p") || invalid;; esac;
m "INPUT_END:$n:${#p}";
{ { ( trap - 0 2 15; m BEGIN >&3; { @EXECUTE@; } 3>&- ) </dev/null 2>/dev/null; m "STATUS:$?"; } | command base64; } 3>&1;
m "END:$?"; park

