#!/usr/bin/env sh

if [ -v $CONTAINER_RUNTIME ];  then
    export CONTAINER_RUNTIME="docker"
fi

rm -rf appimagebuild
mkdir appimagebuild

$CONTAINER_RUNTIME build . --output=./appimagebuild

rm -rf ./appimagebuild/bin
rm -rf ./appimagebuild/etc
rm -rf ./appimagebuild/proc
rm -rf ./appimagebuild/root
rm -rf ./appimagebuild/home
rm -rf ./appimagebuild/dev
rm -rf ./appimagebuild/sbin
rm -rf ./appimagebuild/var
rm -rf ./appimagebuild/media
rm -rf ./appimagebuild/mnt
rm -rf ./appimagebuild/opt
rm -rf ./appimagebuild/run
rm -rf ./appimagebuild/srv
rm -rf ./appimagebuild/sys
rm -rf ./appimagebuild/tmp
rm -rf ./appimagebuild/usr/share
rm -rf ./appimagebuild/usr/sbin
rm -rf ./appimagebuild/usr/local
rm -rf ./appimagebuild/usr/lib/libLLVM.*
rm -rf ./appimagebuild/usr/lib/gallium-pipe
rm -rf ./appimagebuild/usr/lib/libgallium*
rm -rf ./appimagebuild/usr/lib/libgallium*

cp  ./dist/* ./appimagebuild/
if [ ! -e ./appimagetool-x86_64.AppImage ]; then
  wget https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x ./appimagetool-x86_64.AppImage
fi
./appimagetool-x86_64.AppImage appimagebuild pnidgrab.AppImage


